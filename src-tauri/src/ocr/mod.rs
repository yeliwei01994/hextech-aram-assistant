mod monitor;
mod normalize;
mod protocol;

use protocol::{OcrProbeResponse, OcrWorkerRequest, OcrWorkerResponse};
use std::{
    io::{BufRead, BufReader, Write},
    path::{Path, PathBuf},
    process::{Child, ChildStdin, ChildStdout, Command, Stdio},
    time::{Duration, Instant},
};

pub use monitor::start_automatic_detection;
pub use normalize::{is_safe_name_match_for_overlay, match_augment_name};
pub use protocol::{OcrProbeResult, OcrScreenResult};

struct WorkerProcess {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
}

pub struct OcrClient {
    packaged_executable: Option<PathBuf>,
    packaged_layout: Option<PathBuf>,
    development_dir: PathBuf,
    process: Option<WorkerProcess>,
    last_activity: Option<Instant>,
}

impl OcrClient {
    pub fn warmup(&mut self) -> Result<(), String> {
        self.ensure_worker()?;
        self.last_activity = Some(Instant::now());
        let request = serde_json::to_string(&OcrWorkerRequest { action: "warmup".to_string(), language: "ch".to_string() })
            .map_err(|error| format!("准备 OCR 请求失败：{error}"))?;
        let response: serde_json::Value = serde_json::from_str(&self.exchange_request(&request)?)
            .map_err(|error| format!("解析 OCR 初始化结果失败：{error}"))?;
        if let Some(error) = response.get("error").and_then(serde_json::Value::as_str) { return Err(error.to_string()); }
        if response.get("ready").and_then(serde_json::Value::as_bool) != Some(true) { return Err("OCR 初始化没有完成".to_string()); }
        Ok(())
    }
    pub fn new(
        packaged_executable: Option<PathBuf>,
        packaged_layout: Option<PathBuf>,
        development_dir: PathBuf,
    ) -> Self {
        Self {
            packaged_executable,
            packaged_layout,
            development_dir,
            process: None,
            last_activity: None,
        }
    }

    pub fn recognize_current_screen(&mut self) -> Result<OcrScreenResult, String> {
        self.ensure_worker()?;
        self.last_activity = Some(Instant::now());
        let request = serde_json::to_string(&OcrWorkerRequest {
            action: "recognize_current_screen".to_string(),
            language: "ch".to_string(),
        })
        .map_err(|error| format!("准备 OCR 请求失败：{error}"))?;

        let read_result = (|| {
            let process = self.process.as_mut().expect("worker was started");
            writeln!(process.stdin, "{request}")
                .and_then(|_| process.stdin.flush())
                .map_err(|error| format!("发送 OCR 请求失败：{error}"))?;
            let mut line = String::new();
            process
                .stdout
                .read_line(&mut line)
                .map_err(|error| format!("读取 OCR 结果失败：{error}"))?;
            Ok::<String, String>(line)
        })();
        let line = match read_result {
            Ok(line) => line,
            Err(error) => {
                self.reset_worker();
                return Err(error);
            }
        };
        if line.trim().is_empty() {
            self.reset_worker();
            return Err("OCR 进程已退出，请检查本地识别组件是否安装完整。".to_string());
        }

        let response: OcrWorkerResponse = match serde_json::from_str(&line) {
            Ok(response) => response,
            Err(error) => {
                self.reset_worker();
                return Err(format!("解析 OCR 结果失败：{error}"));
            }
        };
        if let Some(error) = response.error {
            return Err(error);
        }
        Ok(OcrScreenResult {
            screen_width: response.screen_width,
            screen_height: response.screen_height,
            monitor: response.monitor,
            game_window: response.game_window,
            cards: response.cards,
        })
    }

    pub fn probe_current_screen(&mut self) -> Result<OcrProbeResult, String> {
        self.ensure_worker()?;
        self.last_activity = Some(Instant::now());
        let request = serde_json::to_string(&OcrWorkerRequest {
            action: "probe_augment_screen".to_string(),
            language: "ch".to_string(),
        })
        .map_err(|error| format!("准备画面探测请求失败：{error}"))?;
        let line = self.exchange_request(&request)?;
        let response: OcrProbeResponse = serde_json::from_str(&line)
            .map_err(|error| format!("解析画面探测结果失败：{error}"))?;
        if let Some(error) = response.error {
            return Err(error);
        }
        Ok(OcrProbeResult {
            game_foreground: response.game_foreground,
            augment_choice: response.augment_choice,
            card_metrics: response.card_metrics,
            game_window: response.game_window,
        })
    }

    fn exchange_request(&mut self, request: &str) -> Result<String, String> {
        let read_result = (|| {
            let process = self.process.as_mut().expect("worker was started");
            writeln!(process.stdin, "{request}")
                .and_then(|_| process.stdin.flush())
                .map_err(|error| format!("发送 OCR 请求失败：{error}"))?;
            let mut line = String::new();
            process
                .stdout
                .read_line(&mut line)
                .map_err(|error| format!("读取 OCR 结果失败：{error}"))?;
            Ok::<String, String>(line)
        })();
        let line = match read_result {
            Ok(line) => line,
            Err(error) => {
                self.reset_worker();
                return Err(error);
            }
        };
        if line.trim().is_empty() {
            self.reset_worker();
            return Err("OCR 进程已退出，请检查本地识别组件是否安装完整。".to_string());
        }
        Ok(line)
    }

    fn ensure_worker(&mut self) -> Result<(), String> {
        let worker_is_running = self
            .process
            .as_mut()
            .is_some_and(|process| matches!(process.child.try_wait(), Ok(None)));
        if worker_is_running {
            return Ok(());
        }
        self.reset_worker();

        if let Some(executable) = self
            .packaged_executable
            .as_ref()
            .filter(|path| !cfg!(debug_assertions) && path.is_file())
        {
            let layout = self
                .packaged_layout
                .as_ref()
                .filter(|path| path.is_file())
                .ok_or_else(|| "安装包缺少 OCR 裁切配置。".to_string())?;
            let mut command = Command::new(executable);
            command.arg("--layout").arg(layout);
            command.current_dir(executable.parent().unwrap_or(Path::new(".")));
            self.process = Some(spawn_worker(command)?);
            return Ok(());
        }

        let script = self.development_dir.join("worker.py");
        let layout = self.development_dir.join("card-layout.json");
        if !script.is_file() || !layout.is_file() {
            return Err(
                "找不到 OCR 识别组件。请重新安装应用，或在开发环境运行 workers/ocr/build.ps1。"
                    .to_string(),
            );
        }

        let project_python = self
            .development_dir
            .join(".venv")
            .join("Scripts")
            .join("python.exe");
        let mut python = if project_python.is_file() {
            Command::new(project_python)
        } else {
            Command::new("python")
        };
        python.arg(&script).arg("--layout").arg(&layout);
        match spawn_worker(python) {
            Ok(process) => {
                self.process = Some(process);
                Ok(())
            }
            Err(first_error) => {
                let mut py_launcher = Command::new("py");
                py_launcher
                    .arg("-3")
                    .arg(&script)
                    .arg("--layout")
                    .arg(&layout);
                spawn_worker(py_launcher)
                    .map(|process| self.process = Some(process))
                    .map_err(|_| {
                        format!(
                            "无法启动本地 OCR。请安装 Python 3.12 与 workers/ocr/requirements.txt 依赖。{first_error}"
                        )
                    })
            }
        }
    }

    fn reset_worker(&mut self) {
        if let Some(mut process) = self.process.take() {
            let _ = process.child.kill();
            let _ = process.child.wait();
        }
    }

    pub fn shutdown(&mut self) {
        self.reset_worker();
        self.last_activity = None;
    }

    pub fn shutdown_if_idle(&mut self, idle_for: Duration) {
        if self
            .last_activity
            .is_some_and(|last| last.elapsed() >= idle_for)
        {
            self.shutdown();
        }
    }
}

impl Drop for OcrClient {
    fn drop(&mut self) {
        if let Some(process) = &mut self.process {
            let _ = process.child.kill();
            let _ = process.child.wait();
        }
    }
}

fn spawn_worker(mut command: Command) -> Result<WorkerProcess, String> {
    command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(if cfg!(debug_assertions) { Stdio::inherit() } else { Stdio::null() });
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    let mut child = command
        .spawn()
        .map_err(|error| format!("启动 OCR 子进程失败：{error}"))?;
    let stdin = child
        .stdin
        .take()
        .ok_or_else(|| "无法连接 OCR 子进程输入。".to_string())?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "无法连接 OCR 子进程输出。".to_string())?;
    Ok(WorkerProcess {
        child,
        stdin,
        stdout: BufReader::new(stdout),
    })
}
