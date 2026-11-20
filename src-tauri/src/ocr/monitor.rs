use crate::AppState;
use std::{
    sync::atomic::Ordering,
    thread,
    time::{Duration, Instant},
};
use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, RefreshKind, System};
use tauri::{AppHandle, Emitter, Manager};

const PROBE_INTERVAL: Duration = Duration::from_millis(300);
const PROBE_ERROR_COOLDOWN: Duration = Duration::from_secs(5);
const POSITIVE_PROBE_WINDOW: Duration = Duration::from_secs(1);
const OCR_RETRY_DELAY: Duration = Duration::from_secs(2);

fn hide_overlay(app: &AppHandle) {
    if let Err(error) = crate::commands::ocr::hide_ocr_overlay(app.clone()) {
        eprintln!("海克斯浮层：{error}");
    }
}

fn should_request_ocr(
    positive_probes: u8,
    stable_signature_probes: u8,
    last_requested: Option<&str>,
    signature: &str,
) -> bool {
    positive_probes >= 2 && match last_requested {
        // A confirmed card layout is enough for the first read. Hover effects
        // can change an individual title hash without changing the layout.
        None => true,
        Some(previous) => stable_signature_probes >= 2 && previous != signature,
    }
}

pub fn start_automatic_detection(app: AppHandle) {
    #[cfg(windows)]
    if cfg!(debug_assertions)
    {
        let app = app.clone();
        thread::Builder::new()
            .name("aram-augment-selection".to_string())
            .spawn(move || {
                let mut was_down = super::selection::input::left_down();
                loop {
                    let down = super::selection::input::left_down();
                    if super::selection::input::escape_down() {
                        if let Ok(mut selection) = app.state::<AppState>().selection.lock() {
                            selection.observe_escape();
                        }
                    }
                    if down && !was_down {
                        if let Ok(mut selection) = app.state::<AppState>().selection.lock() {
                            selection.sample_click();
                        }
                    }
                    was_down = down;
                    thread::sleep(Duration::from_millis(12));
                }
            })
            .expect("failed to start augment selection tracker");
    }
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
    let mut shop_hits = 0u8;
    let mut shop_misses = 0u8;
    let mut shop_open = false;
    let mut augment_visible = false;
    let mut positive_probes = 0u8;
    let mut negative_probes = 0u8;
    let mut foreground_misses = 0u8;
    let mut choice_seen = false;
    let mut last_positive_probe: Option<Instant> = None;
    let mut candidate_signature: Option<String> = None;
    let mut stable_signature_probes = 0u8;
    let mut last_requested_signature: Option<String> = None;
    let mut retry_probe_after = None;
    let mut retry_ocr_after = None;
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
            app.state::<AppState>().ocr_dismissed.store(false, Ordering::Relaxed);
            update_shop_overlay(&app, None, false, &mut shop_hits, &mut shop_misses, &mut shop_open);
            if game_was_running {
                let state = app.state::<AppState>();
                if let Ok(mut selection) = state.selection.lock() {
                    *selection = Default::default();
                }
                if let Ok(mut ocr) = state.ocr.lock() {
                    ocr.shutdown();
                };
            }
            if augment_visible {
                hide_overlay(&app);
            }
            game_was_running = false;
            augment_visible = false;
            positive_probes = 0;
            negative_probes = 0;
            foreground_misses = 0;
            choice_seen = false;
            last_positive_probe = None;
            candidate_signature = None;
            stable_signature_probes = 0;
            last_requested_signature = None;
            retry_probe_after = None;
            retry_ocr_after = None;
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
        if !shop_open && retry_probe_after.is_some_and(|deadline| now < deadline) {
            thread::sleep(PROBE_INTERVAL);
            continue;
        }

        let state = app.state::<AppState>();
        let probe_result = state
            .ocr
            .lock()
            .map_err(|_| "OCR 识别器暂不可用".to_string())
            .and_then(|mut ocr| ocr.probe_current_screen());
        let shop_bounds = probe_result.as_ref().ok()
            .filter(|probe| probe.game_foreground && probe.shop_visible)
            .and_then(|probe| probe.game_window.as_ref());
        if let Ok(probe) = &probe_result {
            update_shop_overlay(&app, shop_bounds, probe.game_foreground, &mut shop_hits, &mut shop_misses, &mut shop_open);
        }

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
                let signature_preview = current.3.split('|')
                    .map(|slot| slot.chars().take(8).collect::<String>())
                    .collect::<Vec<_>>().join("|");
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
                choice_seen = cfg!(debug_assertions);
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
                foreground_misses = 0;
                if let Ok(mut selection) = state.selection.lock() {
                    selection.observe_highlight(&probe.card_metrics);
                    selection.observe_pointer();
                }
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
                if !state.ocr_dismissed.load(Ordering::Relaxed)
                    && retry_ocr_after.is_none_or(|deadline| now >= deadline)
                    && should_request_ocr(
                    positive_probes, stable_signature_probes,
                    last_requested_signature.as_deref(), &signature,
                ) {
                    eprintln!("海克斯检测：{} 卡，请求识别", probe.card_metrics.len());
                    let champion_id = state.ocr_champion_id.lock().ok().and_then(|id| id.clone());
                    let result = crate::commands::ocr::recognize_impl(&app, champion_id, true)
                        .and_then(|payload| match payload {
                            Some(payload) if !state.ocr_dismissed.load(Ordering::Relaxed) =>
                                crate::commands::ocr::show_ocr_overlay(payload, app.clone()).map(|()| true),
                            _ => Ok(false),
                        });
                    if matches!(&result, Ok(true)) {
                        augment_visible = true;
                        if let Ok(mut selection) = state.selection.lock() {
                            selection.observe_highlight(&probe.card_metrics);
                        }
                        last_requested_signature = Some(signature);
                        retry_ocr_after = None;
                    } else {
                        if let Err(error) = result {
                            eprintln!("海克斯自动识别或显示失败：{error}");
                        }
                        retry_ocr_after = Some(Instant::now() + OCR_RETRY_DELAY);
                    }
                    if state.ocr_dismissed.load(Ordering::Relaxed) {
                        hide_overlay(&app);
                    }
                }
            }
            Ok(probe) => {
                retry_probe_after = None;
                if probe.game_foreground {
                    foreground_misses = foreground_misses.saturating_add(1);
                    if foreground_misses == 1 {
                        if let Ok(mut selection) = state.selection.lock() {
                            selection.observe_closing_frame();
                        }
                    }
                    if foreground_misses == 3 {
                        state.ocr_dismissed.store(false, Ordering::Relaxed);
                        retry_ocr_after = None;
                        last_requested_signature = None;
                        if choice_seen {
                            choice_seen = false;
                            if let Ok(mut selection) = state.selection.lock() {
                                selection.confirm_closed_with_hud(probe.hud_selected_name.as_deref());
                            }
                        }
                    }
                } else {
                    foreground_misses = 0;
                    if let Ok(mut selection) = state.selection.lock() {
                        selection.cancel_inference();
                    }
                }
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
                    hide_overlay(&app);
                }
            }
            Err(error) => {
                foreground_misses = 0;
                if let Ok(mut selection) = state.selection.lock() {
                    selection.cancel_inference();
                }
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
                    hide_overlay(&app);
                }
            }
        }

        thread::sleep(PROBE_INTERVAL);
    }
}

#[cfg(test)]
mod request_tests {
    use super::should_request_ocr;

    #[test]
    fn first_read_tolerates_jitter_but_refresh_waits_for_stable_change() {
        let first = "1:a|2:b|3:c|4:d";
        let changed = "1:a|2:b|3:c|4:e";
        assert!(!should_request_ocr(1, 1, None, first));
        assert!(should_request_ocr(2, 1, None, changed));
        assert!(!should_request_ocr(3, 1, Some(first), changed));
        assert!(!should_request_ocr(3, 2, Some(first), first));
        assert!(should_request_ocr(4, 2, Some(first), changed));
    }
}

fn update_shop_overlay(
    app: &AppHandle,
    bounds: Option<&crate::domain::OcrScreenBounds>,
    game_foreground: bool,
    hits: &mut u8,
    misses: &mut u8,
    open: &mut bool,
) {
    let Some(window) = app.get_webview_window("shop-overlay") else { return };
    if let Some(bounds) = bounds {
        *misses = 0;
        *hits = hits.saturating_add(1);
        if *hits < 2 { return; }
        if !*open {
            let shown = crate::commands::ocr::position_overlay(&window, None, Some(bounds))
                .and_then(|()| window.set_ignore_cursor_events(true).map_err(|e| e.to_string()))
                .and_then(|()| window.show().map_err(|e| e.to_string()));
            if let Err(error) = shown {
                eprintln!("显示装备浮层失败：{error}");
                return;
            }
            eprintln!("装备浮层：已显示");
            *open = true;
            let _ = app.emit_to("shop-overlay", "shop:visibility", true);
        }
    } else {
        *hits = 0;
        *misses = misses.saturating_add(1);
        if game_foreground && *misses < 2 { return; }
        if *open {
            let _ = app.emit_to("shop-overlay", "shop:visibility", false);
            if window.hide().is_ok() {
                *open = false;
                eprintln!("装备浮层：已隐藏");
            }
        }
    }
}
