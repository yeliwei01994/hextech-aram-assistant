use crate::AppState;
use std::{
    thread,
    time::{Duration, Instant},
};
use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, RefreshKind, System};
use tauri::{AppHandle, Emitter, Manager};

const PROBE_INTERVAL: Duration = Duration::from_millis(300);
const AUTO_REQUEST_INTERVAL: Duration = Duration::from_secs(3);
const PROBE_ERROR_COOLDOWN: Duration = Duration::from_secs(5);

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
    let mut last_auto_request: Option<Instant> = None;
    let mut retry_probe_after = None;
    let mut last_probe_state: Option<(bool, bool)> = None;

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
            last_auto_request = None;
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
            let current = (probe.game_foreground, probe.augment_choice);
            if last_probe_state != Some(current) {
                eprintln!("海克斯检测：游戏前台={}，卡片界面={}", current.0, current.1);
                last_probe_state = Some(current);
            }
        }

        match probe_result {
            Ok(probe) if probe.game_foreground && probe.augment_choice => {
                retry_probe_after = None;
                positive_probes = positive_probes.saturating_add(1);
                negative_probes = 0;
                if positive_probes >= 2 {
                    augment_visible = true;
                    let should_request = last_auto_request
                        .map_or(true, |last| last.elapsed() >= AUTO_REQUEST_INTERVAL);
                    if should_request {
                        if !augment_visible || last_auto_request.is_none() { eprintln!("海克斯检测：请求识别卡片"); }
                        let _ = app.emit_to("main", "ocr:auto-request", ());
                        last_auto_request = Some(Instant::now());
                    }
                }
            }
            Ok(_) => {
                retry_probe_after = None;
                positive_probes = 0;
                negative_probes = negative_probes.saturating_add(1);
                if augment_visible && negative_probes >= 3 {
                    augment_visible = false;
                    last_auto_request = None;
                    let _ = app.emit_to("main", "ocr:auto-hide", ());
                }
            }
            Err(error) => {
                eprintln!("海克斯自动识别暂不可用：{error}");
                retry_probe_after = Some(Instant::now() + PROBE_ERROR_COOLDOWN);
                positive_probes = 0;
                negative_probes = negative_probes.saturating_add(1);
                if augment_visible && negative_probes >= 3 {
                    augment_visible = false;
                    last_auto_request = None;
                    let _ = app.emit_to("main", "ocr:auto-hide", ());
                }
            }
        }

        thread::sleep(PROBE_INTERVAL);
    }
}
