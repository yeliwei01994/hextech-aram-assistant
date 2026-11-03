use crate::AppState;
use std::{
    thread,
    time::{Duration, Instant},
};
use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, RefreshKind, System};
use tauri::{AppHandle, Emitter, Manager};

const PROBE_INTERVAL: Duration = Duration::from_millis(300);
const PROBE_ERROR_COOLDOWN: Duration = Duration::from_secs(5);
const POSITIVE_PROBE_WINDOW: Duration = Duration::from_secs(1);

pub fn start_automatic_detection(app: AppHandle) {
    thread::Builder::new()
        .name("aram-augment-screen-monitor".to_string())
        .spawn(move || monitor_loop(app))
        .expect("failed to start augment screen monitor");
}

fn monitor_loop(app: AppHandle) {
    let mut system = match System::new_with_specifics(
        RefreshKind::nothing().with_processes(ProcessRefreshKind::nothing()),
    ) {
        Ok(system) => system,
        Err(error) => {
            eprintln!("初始化游戏进程监测失败：{error}");
            return;
        }
    };
    let mut game_was_running = false;
    let mut augment_visible = false;
    let mut positive_probes = 0u8;
    let mut negative_probes = 0u8;
    let mut last_positive_probe: Option<Instant> = None;
    let mut candidate_signature: Option<String> = None;
    let mut stable_signature_probes = 0u8;
    let mut last_requested_signature: Option<String> = None;
    let mut retry_probe_after = None;
    let mut last_probe_state: Option<(bool, bool, usize, String)> = None;

    loop {
        system.refresh_processes_specifics(
            ProcessesToUpdate::All,
            true,
            ProcessRefreshKind::nothing(),
        );
        let game_running = system.processes().values().any(|process| {
            process
                .name()
                .to_string_lossy()
                .eq_ignore_ascii_case("League of Legends.exe")
        });

        if !game_running {
            if game_was_running {
                let state = app.state::<AppState>();
                if let Ok(mut ocr) = state.ocr.lock() {
                    ocr.shutdown();
                };
            }
            if augment_visible {
                let _ = app.emit_to("main", "ocr:auto-hide", ());
            }
            game_was_running = false;
            augment_visible = false;
            positive_probes = 0;
            negative_probes = 0;
            last_positive_probe = None;
            candidate_signature = None;
            stable_signature_probes = 0;
            last_requested_signature = None;
            retry_probe_after = None;
            last_probe_state = None;
            thread::sleep(PROBE_INTERVAL);
            continue;
        }
        if !game_was_running {
            eprintln!("海克斯检测：已发现游戏进程，正在预热 OCR");
            let state = app.state::<AppState>();
            if let Ok(mut ocr) = state.ocr.lock() {
                match ocr.warmup() {
                    Ok(()) => eprintln!("海克斯检测：OCR 已准备"),
                    Err(error) => eprintln!("海克斯 OCR 初始化失败：{error}"),
                }
            };
        }
        game_was_running = true;

        let now = Instant::now();
        if retry_probe_after.is_some_and(|deadline| now < deadline) {
            thread::sleep(PROBE_INTERVAL);
            continue;
        }

        let state = app.state::<AppState>();
        let probe_result = state
            .ocr
            .lock()
            .map_err(|_| "OCR 识别器暂不可用".to_string())
            .and_then(|mut ocr| ocr.probe_current_screen());

        if let Ok(probe) = &probe_result {
            let matching_cards = probe.card_metrics.iter().filter(|card| card.matched).count();
            let signature = probe
                .card_metrics
                .iter()
                .map(|card| format!("{}:{}", card.slot, card.signature))
                .collect::<Vec<_>>()
                .join("|");
            let current = (
                probe.game_foreground,
                probe.augment_choice,
                matching_cards,
                signature,
            );
            if last_probe_state.as_ref() != Some(&current) {
                let metrics = probe.card_metrics.iter().map(|card| {
                    format!(
                        "{}:{:.2}/{:.2}{}",
                        card.slot,
                        card.bright_border_ratio,
                        card.dark_center_ratio,
                        if card.matched { "✓" } else { "×" },
                    )
                }).collect::<Vec<_>>().join(" ");
                let window = probe.game_window.as_ref().map_or_else(
                    || "未知尺寸".to_string(),
                    |rect| format!("{}x{}", rect.width, rect.height),
                );
                let signature_preview = current.3.chars().take(14).collect::<String>();
                eprintln!(
                    "海克斯检测：游戏前台={}，卡片界面={}，命中 {}/{} [{}]，窗口 {}，指纹 {}",
                    current.0,
                    current.1,
                    matching_cards,
                    probe.card_metrics.len(),
                    metrics,
                    window,
                    signature_preview,
                );
                last_probe_state = Some(current);
            }
        }

        match probe_result {
            Ok(probe) if probe.game_foreground && probe.augment_choice => {
                retry_probe_after = None;
                let now = Instant::now();
                if last_positive_probe
                    .is_none_or(|last| now.duration_since(last) > POSITIVE_PROBE_WINDOW)
                {
                    positive_probes = 0;
                    candidate_signature = None;
                    stable_signature_probes = 0;
                }
                positive_probes = positive_probes.saturating_add(1);
                last_positive_probe = Some(now);
                negative_probes = 0;
                let signature = probe
                    .card_metrics
                    .iter()
                    .map(|card| format!("{}:{}", card.slot, card.signature))
                    .collect::<Vec<_>>()
                    .join("|");
                if candidate_signature.as_deref() == Some(signature.as_str()) {
                    stable_signature_probes = stable_signature_probes.saturating_add(1);
                } else {
                    candidate_signature = Some(signature.clone());
                    stable_signature_probes = 1;
                }
                if positive_probes >= 2 && stable_signature_probes >= 2 {
                    augment_visible = true;
                    if last_requested_signature.as_deref() != Some(signature.as_str()) {
                        eprintln!("海克斯检测：卡片内容变化，立即请求识别");
                        let _ = app.emit_to("main", "ocr:auto-request", ());
                        last_requested_signature = Some(signature);
                    }
                }
            }
            Ok(probe) => {
                retry_probe_after = None;
                // Focus can briefly move away from the game during fullscreen
                // transitions. Keep a recent positive sample for one second,
                // but reset immediately when the game is foreground and the
                // card layout itself does not match.
                if probe.game_foreground {
                    positive_probes = 0;
                    last_positive_probe = None;
                    candidate_signature = None;
                    stable_signature_probes = 0;
                }
                negative_probes = negative_probes.saturating_add(1);
                if augment_visible && negative_probes >= 3 {
                    augment_visible = false;
                    last_requested_signature = None;
                    let _ = app.emit_to("main", "ocr:auto-hide", ());
                }
            }
            Err(error) => {
                eprintln!("海克斯自动识别暂不可用：{error}");
                retry_probe_after = Some(Instant::now() + PROBE_ERROR_COOLDOWN);
                positive_probes = 0;
                last_positive_probe = None;
                candidate_signature = None;
                stable_signature_probes = 0;
                last_requested_signature = None;
                negative_probes = negative_probes.saturating_add(1);
                if augment_visible && negative_probes >= 3 {
                    augment_visible = false;
                    let _ = app.emit_to("main", "ocr:auto-hide", ());
                }
            }
        }

        thread::sleep(PROBE_INTERVAL);
    }
}
