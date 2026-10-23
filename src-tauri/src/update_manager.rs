use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};
use volumectl_lib::updates::{ReleaseInfo, UpdateInfo};

const SUCCESS_CACHE_TTL: Duration = Duration::from_secs(10 * 60);
const PERIODIC_CHECK_INTERVAL: Duration = Duration::from_secs(6 * 60 * 60);
const INITIAL_CHECK_DELAY: Duration = Duration::from_secs(15);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UpdatePhase {
    Idle,
    Checking,
    Available,
    Downloading,
    Ready,
    Installing,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct UpdateState {
    pub revision: u64,
    pub phase: UpdatePhase,
    pub info: Option<UpdateInfo>,
    pub error: Option<String>,
    pub install_supported: bool,
}

impl Default for UpdateState {
    fn default() -> Self {
        Self {
            revision: 0,
            phase: UpdatePhase::Idle,
            info: None,
            error: None,
            install_supported: is_install_supported(),
        }
    }
}

pub struct PreparedPackage {
    pub tag: String,
    pub version: String,
    pub asset_name: String,
    pub asset_path: PathBuf,
    pub manifest_path: PathBuf,
}

pub struct UpdateManager {
    app: AppHandle,
    state: Arc<Mutex<UpdateState>>,
    prepared_package: Arc<Mutex<Option<PreparedPackage>>>,
    is_installing: Arc<AtomicBool>,
    check_trigger: Arc<std::sync::mpsc::SyncSender<bool>>,
}

/// Detect if self-update / installation is supported on this system and install path.
pub fn is_install_supported() -> bool {
    let Ok(current_exe) = std::env::current_exe() else {
        return false;
    };

    #[cfg(windows)]
    {
        if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
            let standard_root = Path::new(&local_app_data).join("VolumeControl");
            if current_exe.starts_with(&standard_root) {
                return true;
            }
        }
        false
    }

    #[cfg(target_os = "macos")]
    {
        let exe_str = current_exe.to_string_lossy();
        if let Some(app_idx) = exe_str.find(".app") {
            let app_bundle = PathBuf::from(&exe_str[..app_idx + 4]);
            if app_bundle.exists() {
                // Check if inside Applications or ~/Applications
                if let Ok(home) = std::env::var("HOME") {
                    let user_apps = Path::new(&home).join("Applications");
                    if app_bundle.starts_with(&user_apps) || app_bundle.starts_with("/Applications")
                    {
                        // Check if writable
                        if let Ok(meta) = app_bundle.metadata() {
                            return !meta.permissions().readonly();
                        }
                    }
                }
            }
        }
        false
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        if let Ok(home) = std::env::var("HOME") {
            let standard_bin = Path::new(&home).join(".local").join("bin");
            if current_exe.starts_with(&standard_bin) {
                return true;
            }
            let receipt = Path::new(&home)
                .join(".local")
                .join("share")
                .join("doc")
                .join("volumecontrol")
                .join("install-receipt.txt");
            if receipt.exists() {
                return true;
            }
        }
        false
    }
}

/// Expected installer asset name for the current platform.
pub fn platform_asset_name(version: &str) -> Option<&'static str> {
    #[cfg(windows)]
    {
        let _ = version;
        // Windows setup asset name: VolumeControl_<version>_x64-setup.exe
        // Returned dynamically or via format helper
        None
    }
    #[cfg(not(windows))]
    {
        let _ = version;
        None
    }
}

pub fn get_platform_asset_filename(version: &str) -> String {
    #[cfg(windows)]
    {
        format!("VolumeControl_{version}_x64-setup.exe")
    }
    #[cfg(target_os = "macos")]
    {
        format!("volumecontrol-{version}-macos.zip")
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        format!("volumecontrol-{version}-linux-amd64.tar.gz")
    }
}

impl UpdateManager {
    pub fn start(app: AppHandle) -> Result<Self, String> {
        let (tx, rx) = std::sync::mpsc::sync_channel::<bool>(8);
        let check_trigger = Arc::new(tx);

        let state = Arc::new(Mutex::new(UpdateState::default()));
        let last_success = Arc::new(Mutex::new(None));
        let prepared_package = Arc::new(Mutex::new(None));
        let is_checking = Arc::new(AtomicBool::new(false));
        let is_downloading = Arc::new(AtomicBool::new(false));
        let is_installing = Arc::new(AtomicBool::new(false));

        let manager = Self {
            app: app.clone(),
            state: state.clone(),
            prepared_package: prepared_package.clone(),
            is_installing: is_installing.clone(),
            check_trigger: check_trigger.clone(),
        };

        // Background worker loop
        let worker_app = app.clone();
        let worker_state = state.clone();
        let worker_last_success = last_success.clone();
        let worker_prepared = prepared_package.clone();
        let worker_is_checking = is_checking.clone();
        let worker_is_downloading = is_downloading.clone();

        std::thread::Builder::new()
            .name("update-manager-worker".into())
            .spawn(move || {
                // Initial check after 15 seconds
                std::thread::sleep(INITIAL_CHECK_DELAY);
                Self::perform_check(
                    &worker_app,
                    worker_state.clone(),
                    worker_last_success.clone(),
                    worker_prepared.clone(),
                    worker_is_checking.clone(),
                    worker_is_downloading.clone(),
                    false,
                );

                // Periodic loop every 6 hours or on manual trigger
                loop {
                    match rx.recv_timeout(PERIODIC_CHECK_INTERVAL) {
                        Ok(force) => {
                            Self::perform_check(
                                &worker_app,
                                worker_state.clone(),
                                worker_last_success.clone(),
                                worker_prepared.clone(),
                                worker_is_checking.clone(),
                                worker_is_downloading.clone(),
                                force,
                            );
                        }
                        Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                            Self::perform_check(
                                &worker_app,
                                worker_state.clone(),
                                worker_last_success.clone(),
                                worker_prepared.clone(),
                                worker_is_checking.clone(),
                                worker_is_downloading.clone(),
                                false,
                            );
                        }
                        Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
                    }
                }
            })
            .map_err(|e| format!("failed to spawn update worker: {e}"))?;

        Ok(manager)
    }

    pub fn snapshot(&self) -> UpdateState {
        self.state.lock().unwrap_or_else(|p| p.into_inner()).clone()
    }

    pub fn request_check(&self, force: bool) -> UpdateState {
        let _ = self.check_trigger.try_send(force);
        self.snapshot()
    }

    fn update_and_emit(
        app: &AppHandle,
        state_lock: &Mutex<UpdateState>,
        update_fn: impl FnOnce(&mut UpdateState),
    ) -> UpdateState {
        let new_state = {
            let mut s = state_lock.lock().unwrap_or_else(|p| p.into_inner());
            update_fn(&mut s);
            s.revision += 1;
            s.clone()
        };
        let _ = app.emit("state://update", &new_state);
        new_state
    }

    fn current_language(app: &AppHandle) -> volumectl_lib::config::Language {
        app.try_state::<Arc<Mutex<volumectl_lib::host_core::AppCore>>>()
            .map(|shared| {
                shared
                    .lock()
                    .unwrap_or_else(|p| p.into_inner())
                    .config()
                    .language
            })
            .unwrap_or_default()
    }

    fn perform_check(
        app: &AppHandle,
        state_arc: Arc<Mutex<UpdateState>>,
        last_success_arc: Arc<Mutex<Option<(Instant, String)>>>,
        prepared_arc: Arc<Mutex<Option<PreparedPackage>>>,
        is_checking: Arc<AtomicBool>,
        is_downloading: Arc<AtomicBool>,
        force: bool,
    ) {
        if is_checking.swap(true, Ordering::SeqCst) {
            // Already checking; coalesced
            return;
        }

        let _guard = defer::Defer::new(|| {
            is_checking.store(false, Ordering::SeqCst);
        });

        let lang = Self::current_language(app);

        // Check 10-minute cache if not forced
        if !force {
            if let Ok(guard) = last_success_arc.lock() {
                if let Some((checked_at, tag)) = guard.as_ref() {
                    if checked_at.elapsed() < SUCCESS_CACHE_TTL {
                        let info = volumectl_lib::updates::resolve_update_info(
                            lang,
                            env!("CARGO_PKG_VERSION"),
                            Ok(tag.clone()),
                        );
                        Self::update_and_emit(app, &state_arc, |s| {
                            s.info = Some(info);
                            s.error = None;
                        });
                        return;
                    }
                }
            }
        }

        // Set Checking phase
        Self::update_and_emit(app, &state_arc, |s| {
            s.phase = UpdatePhase::Checking;
            s.error = None;
        });

        // Perform discovery
        let release = match volumectl_lib::updates::fetch_latest_release() {
            Ok(rel) => rel,
            Err(err) => {
                log::warn!("update check failed: {err}");
                Self::update_and_emit(app, &state_arc, |s| {
                    // If we already have a Ready package, retain Ready phase
                    if s.phase != UpdatePhase::Ready {
                        s.phase = UpdatePhase::Error;
                    }
                    s.error = Some(err);
                });
                return;
            }
        };

        // Cache successful check
        if let Ok(mut guard) = last_success_arc.lock() {
            *guard = Some((Instant::now(), release.tag_name.clone()));
        }

        let info = volumectl_lib::updates::resolve_update_info(
            lang,
            env!("CARGO_PKG_VERSION"),
            Ok(release.tag_name.clone()),
        );

        if !info.is_update_available {
            // Up to date
            Self::update_and_emit(app, &state_arc, |s| {
                s.phase = UpdatePhase::Idle;
                s.info = Some(info);
                s.error = None;
            });
            return;
        }

        // Update is available!
        Self::update_and_emit(app, &state_arc, |s| {
            s.phase = UpdatePhase::Available;
            s.info = Some(info.clone());
            s.error = None;
        });

        // Step 5: Send native notification with deduplication
        Self::notify_update_available(app, &release.tag_name);

        // Dispatch background download
        Self::spawn_download(
            app.clone(),
            state_arc,
            prepared_arc,
            is_downloading,
            release,
        );
    }

    fn notify_update_available(app: &AppHandle, tag: &str) {
        if let Err(e) = crate::lifecycle::notify_update_if_needed(app, tag) {
            log::warn!("failed to send native update notification: {e}");
        }
    }

    fn spawn_download(
        app: AppHandle,
        state_arc: Arc<Mutex<UpdateState>>,
        prepared_arc: Arc<Mutex<Option<PreparedPackage>>>,
        is_downloading: Arc<AtomicBool>,
        release: ReleaseInfo,
    ) {
        if is_downloading.swap(true, Ordering::SeqCst) {
            // Download already in flight
            return;
        }
        std::thread::Builder::new()
            .name("update-downloader".into())
            .spawn(move || {
                let _dl_guard = defer::Defer::new(|| {
                    is_downloading.store(false, Ordering::SeqCst);
                });

                Self::update_and_emit(&app, &state_arc, |s| {
                    s.phase = UpdatePhase::Downloading;
                    s.error = None;
                });

                match Self::download_and_verify(&app, &release) {
                    Ok(prepared) => {
                        if let Ok(mut guard) = prepared_arc.lock() {
                            *guard = Some(prepared);
                        }
                        Self::update_and_emit(&app, &state_arc, |s| {
                            s.phase = UpdatePhase::Ready;
                            s.error = None;
                        });
                    }
                    Err(e) => {
                        log::error!("update download failed: {e}");
                        Self::update_and_emit(&app, &state_arc, |s| {
                            s.phase = UpdatePhase::Error;
                            s.error = Some(e);
                        });
                    }
                }
            })
            .ok();
    }

    fn download_and_verify(
        app: &AppHandle,
        release: &ReleaseInfo,
    ) -> Result<PreparedPackage, String> {
        let tag = release.tag_name.trim();
        let version = tag.strip_prefix('v').unwrap_or(tag);
        let expected_asset_name = get_platform_asset_filename(version);

        let asset = release
            .assets
            .iter()
            .find(|a| a.name == expected_asset_name)
            .ok_or_else(|| format!("release {tag} does not contain asset {expected_asset_name}"))?;

        let manifest = release
            .assets
            .iter()
            .find(|a| a.name == "SHA256SUMS.txt")
            .ok_or_else(|| format!("release {tag} does not contain SHA256SUMS.txt"))?;

        // Storage directory: cache_dir/updates/<tag>/
        let cache_dir = app
            .path()
            .app_cache_dir()
            .map_err(|e| format!("cannot determine app cache dir: {e}"))?;
        let update_dir = cache_dir.join("updates").join(tag);
        std::fs::create_dir_all(&update_dir)
            .map_err(|e| format!("failed to create update dir {}: {e}", update_dir.display()))?;

        let manifest_dest = update_dir.join("SHA256SUMS.txt");
        let manifest_part = update_dir.join("SHA256SUMS.txt.part");

        // Download manifest
        crate::lifecycle::stream_download_file(
            &manifest.browser_download_url,
            &manifest_part,
            &manifest_dest,
        )?;

        // Parse expected checksum from manifest
        let expected_sha256 =
            crate::lifecycle::extract_manifest_sha256(&manifest_dest, &expected_asset_name)?;

        // Download asset with streaming hash check
        let asset_dest = update_dir.join(&expected_asset_name);
        let asset_part = update_dir.join(format!("{expected_asset_name}.part"));

        crate::lifecycle::stream_download_verified_file(
            &asset.browser_download_url,
            &asset_part,
            &asset_dest,
            &expected_sha256,
        )?;

        // Validate architecture / binary headers
        crate::lifecycle::validate_downloaded_payload(&asset_dest)?;

        Ok(PreparedPackage {
            tag: tag.to_string(),
            version: version.to_string(),
            asset_name: expected_asset_name,
            asset_path: asset_dest,
            manifest_path: manifest_dest,
        })
    }

    pub async fn install(&self, expected_version: &str) -> Result<(), String> {
        let snapshot = self.snapshot();
        if snapshot.phase != UpdatePhase::Ready {
            return Err(format!(
                "update is not ready to install (phase: {:?})",
                snapshot.phase
            ));
        }

        let expected_tag = if expected_version.starts_with('v') {
            expected_version.to_string()
        } else {
            format!("v{expected_version}")
        };

        if let Some(info) = &snapshot.info {
            if let Some(latest) = &info.latest_version {
                if latest != &expected_tag {
                    return Err(format!(
                        "expected version '{expected_version}' does not match ready version '{latest}'"
                    ));
                }
            }
        }

        if !snapshot.install_supported {
            return Err(
                "installation not supported for this installation type or directory".to_string(),
            );
        }

        if self.is_installing.swap(true, Ordering::SeqCst) {
            return Err("installation is already in progress".to_string());
        }

        Self::update_and_emit(&self.app, &self.state, |s| {
            s.phase = UpdatePhase::Installing;
            s.error = None;
        });

        let prepared = {
            let guard = self
                .prepared_package
                .lock()
                .unwrap_or_else(|p| p.into_inner());
            guard.as_ref().map(|p| {
                (
                    p.asset_path.clone(),
                    p.manifest_path.clone(),
                    p.version.clone(),
                )
            })
        };

        let Some((asset_path, manifest_path, version)) = prepared else {
            self.is_installing.store(false, Ordering::SeqCst);
            return Err("prepared update package is missing from cache".to_string());
        };

        // Launch offline prepared apply helper and trigger graceful exit
        match crate::lifecycle::launch_prepared_apply(
            &self.app,
            &asset_path,
            &manifest_path,
            &version,
        )
        .await
        {
            Ok(()) => Ok(()),
            Err(e) => {
                self.is_installing.store(false, Ordering::SeqCst);
                Self::update_and_emit(&self.app, &self.state, |s| {
                    s.phase = UpdatePhase::Ready;
                    s.error = Some(e.clone());
                });
                Err(e)
            }
        }
    }
}
mod defer {
    pub struct Defer<F: FnOnce()>(Option<F>);
    impl<F: FnOnce()> Defer<F> {
        pub fn new(f: F) -> Self {
            Self(Some(f))
        }
    }
    impl<F: FnOnce()> Drop for Defer<F> {
        fn drop(&mut self) {
            if let Some(f) = self.0.take() {
                f();
            }
        }
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;

    #[test]
    fn update_phase_serialization() {
        assert_eq!(
            serde_json::to_string(&UpdatePhase::Idle).unwrap(),
            "\"idle\""
        );
        assert_eq!(
            serde_json::to_string(&UpdatePhase::Checking).unwrap(),
            "\"checking\""
        );
        assert_eq!(
            serde_json::to_string(&UpdatePhase::Available).unwrap(),
            "\"available\""
        );
        assert_eq!(
            serde_json::to_string(&UpdatePhase::Downloading).unwrap(),
            "\"downloading\""
        );
        assert_eq!(
            serde_json::to_string(&UpdatePhase::Ready).unwrap(),
            "\"ready\""
        );
        assert_eq!(
            serde_json::to_string(&UpdatePhase::Installing).unwrap(),
            "\"installing\""
        );
        assert_eq!(
            serde_json::to_string(&UpdatePhase::Error).unwrap(),
            "\"error\""
        );
    }

    #[test]
    fn update_state_defaults() {
        let state = UpdateState::default();
        assert_eq!(state.revision, 0);
        assert_eq!(state.phase, UpdatePhase::Idle);
        assert!(state.info.is_none());
        assert!(state.error.is_none());
    }

    #[test]
    fn platform_asset_filename_format() {
        let name = get_platform_asset_filename("0.1.10");
        #[cfg(windows)]
        assert_eq!(name, "VolumeControl_0.1.10_x64-setup.exe");
        #[cfg(target_os = "macos")]
        assert_eq!(name, "volumecontrol-0.1.10-macos.zip");
        #[cfg(all(unix, not(target_os = "macos")))]
        assert_eq!(name, "volumecontrol-0.1.10-linux-amd64.tar.gz");
    }
}
