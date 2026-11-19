use crate::error::{ChronosError, Result};
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::process::{Child, Command};
use tracing::{debug, info, warn};

/// Configuration options for launching a Chrome/Chromium instance.
#[derive(Debug, Clone)]
pub struct LaunchOptions {
    /// Custom path to the Chrome/Chromium executable.
    pub chrome_path: Option<PathBuf>,
    /// User data directory for session persistence. If None, a temporary directory is created.
    pub user_data_dir: Option<PathBuf>,
    /// Whether to run Chrome in headless mode (`--headless=new`). Defaults to true.
    pub headless: bool,
    /// Remote debugging port (0 for dynamic port allocation).
    pub port: u16,
    /// Window width for browser rendering.
    pub window_width: u32,
    /// Window height for browser rendering.
    pub window_height: u32,
    /// Additional command-line flags.
    pub additional_args: Vec<String>,
}

impl Default for LaunchOptions {
    fn default() -> Self {
        Self {
            chrome_path: None,
            user_data_dir: None,
            headless: true,
            port: 0,
            window_width: 1280,
            window_height: 800,
            additional_args: Vec::new(),
        }
    }
}

impl LaunchOptions {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn headless(mut self, headless: bool) -> Self {
        self.headless = headless;
        self
    }

    pub fn user_data_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.user_data_dir = Some(dir.into());
        self
    }

    pub fn chrome_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.chrome_path = Some(path.into());
        self
    }

    pub fn port(mut self, port: u16) -> Self {
        self.port = port;
        self
    }

    pub fn window_size(mut self, width: u32, height: u32) -> Self {
        self.window_width = width;
        self.window_height = height;
        self
    }

    pub fn arg(mut self, arg: impl Into<String>) -> Self {
        self.additional_args.push(arg.into());
        self
    }
}

/// Represents a running Chrome process with CDP debugging enabled.
pub struct ChromeProcess {
    child: Option<Child>,
    ws_url: String,
    user_data_dir: PathBuf,
    _temp_dir: Option<tempfile::TempDir>,
}

impl ChromeProcess {
    /// The WebSocket URL to connect to the browser target.
    pub fn ws_url(&self) -> &str {
        &self.ws_url
    }

    /// The user data directory used by this instance.
    pub fn user_data_dir(&self) -> &Path {
        &self.user_data_dir
    }

    /// The PID of the Chrome process if still running.
    pub fn pid(&self) -> Option<u32> {
        self.child.as_ref().and_then(|c| c.id())
    }

    /// Asynchronously shuts down the Chrome process cleanly.
    pub async fn shutdown(&mut self) -> Result<()> {
        if let Some(mut child) = self.child.take() {
            debug!("Shutting down Chrome process (PID: {:?})", child.id());
            let _ = child.kill().await;
            let _ = child.wait().await;
        }
        Ok(())
    }

    /// Synchronously kill the process (e.g. for drop/abort).
    pub fn kill(&mut self) -> Result<()> {
        if let Some(mut child) = self.child.take() {
            let _ = child.start_kill();
        }
        Ok(())
    }
}

impl Drop for ChromeProcess {
    fn drop(&mut self) {
        if let Some(mut child) = self.child.take() {
            debug!("Dropping ChromeProcess; terminating child PID: {:?}", child.id());
            let _ = child.start_kill();
        }
    }
}

/// Finds an installed Chrome/Chromium binary on the system.
pub fn find_chrome_binary(custom_path: Option<&Path>) -> Result<PathBuf> {
    // 1. Explicit path passed in options
    if let Some(path) = custom_path {
        if path.exists() {
            return Ok(path.to_path_buf());
        } else {
            return Err(ChronosError::ChromeBinaryNotFound(format!(
                "Specified chrome_path does not exist: {}",
                path.display()
            )));
        }
    }

    // 2. Check CHRONOS_CHROME_BIN environment variable
    if let Ok(env_path) = std::env::var("CHRONOS_CHROME_BIN") {
        let path = PathBuf::from(&env_path);
        if path.exists() {
            return Ok(path);
        }
        warn!(
            "CHRONOS_CHROME_BIN was set to '{}' but the binary does not exist",
            env_path
        );
    }

    // 3. Search PATH via `which`
    let candidates = [
        "google-chrome",
        "google-chrome-stable",
        "chromium",
        "chromium-browser",
        "brave",
        "brave-browser",
        "microsoft-edge",
        "microsoft-edge-stable",
    ];

    for name in candidates {
        if let Ok(found) = which::which(name) {
            return Ok(found);
        }
    }

    // 4. Check known platform-specific locations
    #[cfg(target_os = "linux")]
    let default_paths = [
        "/usr/bin/google-chrome",
        "/usr/bin/google-chrome-stable",
        "/usr/bin/chromium",
        "/usr/bin/chromium-browser",
        "/usr/bin/brave-browser",
        "/snap/bin/chromium",
        "/snap/bin/brave",
    ];

    #[cfg(target_os = "macos")]
    let default_paths = [
        "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
        "/Applications/Chromium.app/Contents/MacOS/Chromium",
        "/Applications/Brave Browser.app/Contents/MacOS/Brave Browser",
        "/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge",
    ];

    #[cfg(target_os = "windows")]
    let default_paths = [
        r"C:\Program Files\Google\Chrome\Application\chrome.exe",
        r"C:\Program Files (x86)\Google\Chrome\Application\chrome.exe",
        r"C:\Program Files\BraveSoftware\Brave-Browser\Application\brave.exe",
        r"C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe",
    ];

    for path_str in default_paths {
        let path = PathBuf::from(path_str);
        if path.exists() {
            return Ok(path);
        }
    }

    Err(ChronosError::ChromeBinaryNotFound(
        "Could not find Chrome, Chromium, or Brave on this system. Set CHRONOS_CHROME_BIN to specify the executable.".into(),
    ))
}

/// Launches a Chrome instance with the specified options and returns a `ChromeProcess`.
pub async fn launch(options: LaunchOptions) -> Result<ChromeProcess> {
    let binary = find_chrome_binary(options.chrome_path.as_deref())?;
    debug!("Using Chrome binary: {}", binary.display());

    let (user_data_path, temp_dir) = match options.user_data_dir {
        Some(dir) => {
            tokio::fs::create_dir_all(&dir).await?;
            (dir, None)
        }
        None => {
            let tmp = tempfile::Builder::new()
                .prefix("chronos-profile-")
                .tempdir()?;
            let path = tmp.path().to_path_buf();
            (path, Some(tmp))
        }
    };

    // Remove any stale DevToolsActivePort file from a previous aborted session
    let active_port_file = user_data_path.join("DevToolsActivePort");
    if active_port_file.exists() {
        let _ = tokio::fs::remove_file(&active_port_file).await;
    }

    let mut cmd = Command::new(&binary);
    cmd.kill_on_drop(true);

    cmd.arg(format!("--remote-debugging-port={}", options.port));
    cmd.arg(format!("--user-data-dir={}", user_data_path.display()));

    if options.headless {
        cmd.arg("--headless=new");
    }

    // Essential Chrome optimization & sandbox flags
    cmd.arg("--disable-gpu");
    cmd.arg("--no-first-run");
    cmd.arg("--no-default-browser-check");
    cmd.arg("--disable-background-networking");
    cmd.arg("--disable-background-timer-throttling");
    cmd.arg("--disable-backgrounding-occluded-windows");
    cmd.arg("--disable-breakpad");
    cmd.arg("--disable-client-side-phishing-detection");
    cmd.arg("--disable-component-update");
    cmd.arg("--disable-default-apps");
    cmd.arg("--disable-dev-shm-usage");
    cmd.arg("--disable-extensions");
    cmd.arg("--disable-hang-monitor");
    cmd.arg("--disable-ipc-flooding-protection");
    cmd.arg("--disable-popup-blocking");
    cmd.arg("--disable-prompt-on-repost");
    cmd.arg("--disable-renderer-backgrounding");
    cmd.arg("--disable-sync");
    cmd.arg("--renderer-process-limit=1");
    cmd.arg("--js-flags=--max-old-space-size=128");
    cmd.arg("--disk-cache-size=10485760");
    cmd.arg("--media-cache-size=10485760");
    cmd.arg("--disable-features=Translate,OptimizationHints,MediaRouter,DialMediaRouteProvider,AudioServiceOutOfProcess");
    cmd.arg("--force-color-profile=srgb");
    cmd.arg("--metrics-recording-only");
    cmd.arg("--password-store=basic");
    cmd.arg("--use-mock-keychain");
    cmd.arg(format!(
        "--window-size={},{}",
        options.window_width, options.window_height
    ));

    for arg in &options.additional_args {
        cmd.arg(arg);
    }

    cmd.arg("about:blank");

    // Standard I/O config
    cmd.stdout(std::process::Stdio::null());
    cmd.stderr(std::process::Stdio::null());

    let mut child = cmd.spawn().map_err(|e| {
        ChronosError::ProcessLaunchFailed(format!(
            "Failed to spawn Chrome at {}: {}",
            binary.display(),
            e
        ))
    })?;

    info!(
        "Launched Chrome (PID: {:?}, headless: {})",
        child.id(),
        options.headless
    );

    // Poll for DevToolsActivePort
    let timeout = Duration::from_secs(15);
    let start = std::time::Instant::now();
    let poll_interval = Duration::from_millis(50);

    let ws_url = loop {
        if start.elapsed() > timeout {
            let _ = child.kill().await;
            return Err(ChronosError::DevToolsPortNotFound(format!(
                "Timed out waiting for DevToolsActivePort in {}",
                user_data_path.display()
            )));
        }

        // Check if Chrome process died early
        match child.try_wait() {
            Ok(Some(status)) => {
                return Err(ChronosError::ProcessLaunchFailed(format!(
                    "Chrome process exited unexpectedly with status: {}",
                    status
                )));
            }
            Ok(None) => {}
            Err(e) => {
                return Err(ChronosError::ProcessLaunchFailed(format!(
                    "Failed to check Chrome process status: {}",
                    e
                )));
            }
        }

        if active_port_file.exists() {
            if let Ok(content) = tokio::fs::read_to_string(&active_port_file).await {
                let lines: Vec<&str> = content.lines().collect();
                if lines.len() >= 2 {
                    let port_str = lines[0].trim();
                    let path_str = lines[1].trim();

                    if let Ok(port) = port_str.parse::<u16>() {
                        let path = if path_str.starts_with('/') {
                            path_str.to_string()
                        } else {
                            format!("/{}", path_str)
                        };
                        let url = format!("ws://127.0.0.1:{}{}", port, path);
                        debug!("Discovered DevTools WebSocket URL: {}", url);
                        break url;
                    }
                }
            }
        }

        tokio::time::sleep(poll_interval).await;
    };

    Ok(ChromeProcess {
        child: Some(child),
        ws_url,
        user_data_dir: user_data_path,
        _temp_dir: temp_dir,
    })
}

/// Convenience function to launch Chrome with default options.
pub async fn launch_default() -> Result<ChromeProcess> {
    launch(LaunchOptions::default()).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_chrome_binary() {
        let binary = find_chrome_binary(None);
        assert!(binary.is_ok(), "Expected to find system Chrome/Chromium/Brave");
    }

    #[test]
    fn test_launch_options_builder() {
        let options = LaunchOptions::new()
            .headless(false)
            .port(9222)
            .window_size(1920, 1080)
            .arg("--mute-audio");
        assert!(!options.headless);
        assert_eq!(options.port, 9222);
        assert_eq!(options.window_width, 1920);
        assert_eq!(options.window_height, 1080);
        assert_eq!(options.additional_args, vec!["--mute-audio"]);
    }
}

