use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tauri::{AppHandle, Manager};
use tauri_plugin_notification::NotificationExt;

pub fn mark_exit_requested() {
    crate::mark_exit_requested();
}

pub fn request_exit(app: &AppHandle) {
    mark_exit_requested();
    app.exit(0);
}

#[cfg(not(windows))]
pub fn install_signal_handlers(app: AppHandle) -> Result<(), String> {
    use signal_hook::consts::signal::{SIGINT, SIGTERM};
    use signal_hook::iterator::Signals;

    let mut signals = Signals::new([SIGINT, SIGTERM])
        .map_err(|e| format!("failed to register signal handlers: {e}"))?;

    std::thread::Builder::new()
        .name("unix-signal-handler".into())
        .spawn(move || {
            if let Some(sig) = signals.forever().next() {
                log::info!("received termination signal: {sig}, exiting gracefully");
                request_exit(&app);
            }
        })
        .map_err(|e| format!("failed to spawn signal thread: {e}"))?;

    Ok(())
}

#[cfg(windows)]
pub fn install_signal_handlers(_app: AppHandle) -> Result<(), String> {
    Ok(())
}

#[derive(Debug, Serialize, Deserialize, Default)]
struct PersistedUpdateState {
    last_notified_version: Option<String>,
}

fn update_state_path() -> Option<PathBuf> {
    Some(volumectl_lib::config::config_path().with_file_name("update-state.json"))
}

pub fn load_last_notified_version() -> Option<String> {
    let path = update_state_path()?;
    let content = std::fs::read_to_string(path).ok()?;
    let state: PersistedUpdateState = serde_json::from_str(&content).ok()?;
    state.last_notified_version
}

pub fn store_last_notified_version(tag: &str) {
    if let Some(path) = update_state_path() {
        let state = PersistedUpdateState {
            last_notified_version: Some(tag.to_string()),
        };
        if let Ok(json) = serde_json::to_string_pretty(&state) {
            let _ = std::fs::write(path, json);
        }
    }
}

pub fn notify_update_if_needed(app: &AppHandle, tag: &str) -> Result<(), String> {
    if let Some(last) = load_last_notified_version() {
        if last == tag {
            return Ok(());
        }
    }

    #[cfg(target_os = "macos")]
    {
        if !request_macos_notification_permission() {
            return Err("notification permission denied on macOS".to_string());
        }
    }

    let title = "VolumeControl Update Available";
    let body = format!("Version {tag} is available and downloading in the background.");

    app.notification()
        .builder()
        .title(title)
        .body(body)
        .show()
        .map_err(|e| format!("failed to show notification: {e}"))?;

    store_last_notified_version(tag);
    Ok(())
}

#[cfg(target_os = "macos")]
fn request_macos_notification_permission() -> bool {
    use block2::RcBlock;
    use objc2::runtime::Bool;
    use objc2_foundation::NSError;
    use objc2_user_notifications::{UNAuthorizationOptions, UNUserNotificationCenter};
    use std::sync::mpsc;

    let (tx, rx) = mpsc::channel();
    let handler = RcBlock::new(move |granted: Bool, _error: *mut NSError| {
        let _ = tx.send(granted.as_bool());
    });
    let center = UNUserNotificationCenter::currentNotificationCenter();
    unsafe {
        center.requestAuthorizationWithOptions_completionHandler(
            UNAuthorizationOptions::Alert | UNAuthorizationOptions::Sound,
            &handler,
        );
    }
    rx.recv_timeout(Duration::from_secs(10)).unwrap_or(false)
}

pub fn stream_download_file(url: &str, part_path: &Path, final_path: &Path) -> Result<(), String> {
    let response = ureq::get(url)
        .timeout(Duration::from_secs(300))
        .call()
        .map_err(|e| format!("download failed for {url}: {e}"))?;

    let mut reader = response.into_reader();
    let mut file = std::fs::File::create(part_path)
        .map_err(|e| format!("failed to create {}: {e}", part_path.display()))?;

    std::io::copy(&mut reader, &mut file)
        .map_err(|e| format!("failed to write {}: {e}", part_path.display()))?;

    file.flush()
        .map_err(|e| format!("failed to flush {}: {e}", part_path.display()))?;
    drop(file);

    std::fs::rename(part_path, final_path)
        .map_err(|e| format!("failed to rename to {}: {e}", final_path.display()))?;
    Ok(())
}

pub fn stream_download_verified_file(
    url: &str,
    part_path: &Path,
    final_path: &Path,
    expected_sha256: &str,
) -> Result<(), String> {
    let response = ureq::get(url)
        .timeout(Duration::from_secs(300))
        .call()
        .map_err(|e| format!("download failed for {url}: {e}"))?;

    let mut reader = response.into_reader();
    let mut file = std::fs::File::create(part_path)
        .map_err(|e| format!("failed to create {}: {e}", part_path.display()))?;

    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 64 * 1024];

    loop {
        let bytes_read = reader
            .read(&mut buffer)
            .map_err(|e| format!("read error downloading {url}: {e}"))?;
        if bytes_read == 0 {
            break;
        }
        hasher.update(&buffer[..bytes_read]);
        file.write_all(&buffer[..bytes_read])
            .map_err(|e| format!("write error writing {}: {e}", part_path.display()))?;
    }

    file.flush()
        .map_err(|e| format!("flush error for {}: {e}", part_path.display()))?;
    drop(file);

    let actual_sha256 = format!("{:x}", hasher.finalize());
    if !actual_sha256.eq_ignore_ascii_case(expected_sha256) {
        let _ = std::fs::remove_file(part_path);
        return Err(format!(
            "checksum mismatch for {url}: expected {expected_sha256}, got {actual_sha256}"
        ));
    }

    std::fs::rename(part_path, final_path)
        .map_err(|e| format!("failed to rename to {}: {e}", final_path.display()))?;
    Ok(())
}

pub fn extract_manifest_sha256(manifest_path: &Path, asset_name: &str) -> Result<String, String> {
    let content = std::fs::read_to_string(manifest_path)
        .map_err(|e| format!("failed to read manifest {}: {e}", manifest_path.display()))?;

    let mut matched_digest = None;
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let parts: Vec<&str> = line.split("  ").collect();
        if parts.len() != 2 {
            continue;
        }
        let digest = parts[0].trim();
        let name = parts[1].trim();
        if digest.len() == 64 && digest.chars().all(|c| c.is_ascii_hexdigit()) && name == asset_name
        {
            if matched_digest.is_some() {
                return Err(format!("duplicate manifest entry for {asset_name}"));
            }
            matched_digest = Some(digest.to_lowercase());
        }
    }

    matched_digest.ok_or_else(|| format!("no canonical SHA-256 entry for {asset_name} in manifest"))
}

pub fn validate_downloaded_payload(asset_path: &Path) -> Result<(), String> {
    let mut file = std::fs::File::open(asset_path)
        .map_err(|e| format!("cannot open asset {}: {e}", asset_path.display()))?;
    let mut header = [0u8; 16];
    let n = file
        .read(&mut header)
        .map_err(|e| format!("cannot read asset header: {e}"))?;
    if n < 4 {
        return Err("payload file too short".to_string());
    }

    #[cfg(windows)]
    {
        if &header[..2] != b"MZ" {
            return Err(
                "downloaded asset is not a valid Windows executable (missing MZ signature)"
                    .to_string(),
            );
        }
    }

    #[cfg(target_os = "macos")]
    {
        if &header[..4] != b"PK\x03\x04" {
            return Err(
                "downloaded asset is not a valid zip archive (missing PK signature)".to_string(),
            );
        }
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        if &header[..2] != b"\x1f\x8b" {
            return Err(
                "downloaded asset is not a valid tar.gz archive (missing gzip signature)"
                    .to_string(),
            );
        }
    }

    Ok(())
}

const EMBEDDED_INSTALLER_SCRIPT: &str = {
    #[cfg(windows)]
    {
        include_str!("../../scripts/Install-VolumeControl.ps1")
    }
    #[cfg(target_os = "macos")]
    {
        include_str!("../../packaging/install-macos.sh")
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        include_str!("../../packaging/install-linux.sh")
    }
};

pub async fn launch_prepared_apply(
    app: &AppHandle,
    asset_path: &Path,
    _manifest_path: &Path,
    version: &str,
) -> Result<(), String> {
    let _ = version;
    let prepared_dir = asset_path
        .parent()
        .ok_or_else(|| "asset has no parent directory".to_string())?;
    let cache_dir = app
        .path()
        .app_cache_dir()
        .map_err(|e| format!("cannot resolve cache dir: {e}"))?;
    let helper_dir = cache_dir.join("apply-helper");
    std::fs::create_dir_all(&helper_dir)
        .map_err(|e| format!("failed to create helper dir: {e}"))?;

    let ready_marker = prepared_dir.join("handoff.ready");
    if ready_marker.exists() {
        let _ = std::fs::remove_file(&ready_marker);
    }

    let parent_pid = std::process::id();
    let apply_log = prepared_dir.join("apply.log");
    let log_file = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(&apply_log)
        .map_err(|e| format!("cannot open apply log: {e}"))?;

    #[cfg(windows)]
    {
        let script_path = helper_dir.join("Install-VolumeControl.ps1");
        std::fs::write(&script_path, EMBEDDED_INSTALLER_SCRIPT)
            .map_err(|e| format!("cannot write helper script: {e}"))?;

        let tag = if version.starts_with('v') {
            version.to_string()
        } else {
            format!("v{version}")
        };

        // Launch powershell with separate arguments, detached
        let child = std::process::Command::new("powershell.exe")
            .arg("-NoProfile")
            .arg("-ExecutionPolicy")
            .arg("Bypass")
            .arg("-File")
            .arg(&script_path)
            .arg("-Action")
            .arg("update")
            .arg("-Version")
            .arg(&tag)
            .arg("-PreparedDirectory")
            .arg(prepared_dir.as_os_str())
            .arg("-ParentProcessId")
            .arg(parent_pid.to_string())
            .arg("-Restart")
            .stdout(log_file.try_clone().map_err(|e| e.to_string())?)
            .stderr(log_file)
            .spawn()
            .map_err(|e| format!("failed to spawn update helper: {e}"))?;

        log::info!("spawned update helper child PID {}", child.id());
    }

    #[cfg(unix)]
    {
        let script_path = helper_dir.join("apply.sh");
        std::fs::write(&script_path, EMBEDDED_INSTALLER_SCRIPT)
            .map_err(|e| format!("cannot write helper script: {e}"))?;

        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&script_path)
            .map_err(|e| e.to_string())?
            .permissions();
        perms.set_mode(0o755);
        let _ = std::fs::set_permissions(&script_path, perms);

        let child = std::process::Command::new("bash")
            .arg(&script_path)
            .arg("update")
            .arg("--prepared-dir")
            .arg(prepared_dir.as_os_str())
            .arg("--parent-pid")
            .arg(parent_pid.to_string())
            .arg("--restart")
            .stdout(log_file.try_clone().map_err(|e| e.to_string())?)
            .stderr(log_file)
            .spawn()
            .map_err(|e| format!("failed to spawn update helper: {e}"))?;

        log::info!("spawned update helper child PID {}", child.id());
    }

    // Wait up to 15 seconds for handoff.ready off the async runtime worker thread
    let marker_path = ready_marker.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let start = Instant::now();
        let timeout = Duration::from_secs(15);
        while !marker_path.exists() {
            if start.elapsed() > timeout {
                return Err("timeout waiting for update helper readiness marker".to_string());
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        Ok(())
    })
    .await
    .map_err(|e| format!("readiness wait task failed: {e}"))??;
    log::info!("handoff.ready observed, requesting graceful app exit");
    request_exit(app);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_manifest_sha256_finds_single_entry() {
        let tmp = std::env::temp_dir().join(format!("manifest-test-{}", uuid_stub()));
        std::fs::create_dir_all(&tmp).unwrap();
        let manifest = tmp.join("SHA256SUMS.txt");
        let content =
            "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef  my-asset.exe\n";
        std::fs::write(&manifest, content).unwrap();

        let hash = extract_manifest_sha256(&manifest, "my-asset.exe").unwrap();
        assert_eq!(
            hash,
            "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
        );

        let missing = extract_manifest_sha256(&manifest, "other.exe");
        assert!(missing.is_err());

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn extract_manifest_sha256_rejects_duplicates() {
        let tmp = std::env::temp_dir().join(format!("manifest-dup-{}", uuid_stub()));
        std::fs::create_dir_all(&tmp).unwrap();
        let manifest = tmp.join("SHA256SUMS.txt");
        let content = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef  my-asset.exe\n0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef  my-asset.exe\n";
        std::fs::write(&manifest, content).unwrap();

        let dup = extract_manifest_sha256(&manifest, "my-asset.exe");
        assert!(dup.is_err());
        assert!(dup.unwrap_err().contains("duplicate"));

        let _ = std::fs::remove_dir_all(&tmp);
    }

    fn uuid_stub() -> u128 {
        use std::time::SystemTime;
        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    }
}
