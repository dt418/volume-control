//! Host-owned update download, verification, and install launch.
//!
//! Mirrors the Tauri host contract (`src-tauri/src/update_manager.rs`): resolve
//! the platform asset from the latest release, download it with a streaming
//! SHA-256 check against `SHA256SUMS.txt`, validate the payload magic, then
//! hand the verified installer to a detached helper after the host exits.

use std::io::Read;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

/// Installer filename for the current platform.
#[must_use]
pub fn asset_filename(version: &str) -> String {
    let arch = match std::env::consts::ARCH {
        "aarch64" | "arm64" => "arm64",
        _ => "x86_64",
    };
    #[cfg(target_os = "windows")]
    {
        format!("volumecontrol-{version}-windows-{arch}-setup.exe")
    }
    #[cfg(target_os = "macos")]
    {
        format!("volumecontrol-{version}-macos-{arch}.dmg")
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        format!("volumecontrol-{version}-linux-{arch}.tar.gz")
    }
}

/// Directory holding downloaded packages: `%LOCALAPPDATA%/VolumeControl/updates/<tag>`.
#[must_use]
pub fn package_dir(tag: &str) -> PathBuf {
    let root = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(PathBuf::from))
        .unwrap_or_else(|| PathBuf::from("."));
    root.join("VolumeControl").join("updates").join(tag)
}

/// Extract the expected SHA-256 for `asset_name` from a `SHA256SUMS.txt` body.
#[must_use]
pub fn manifest_sha256(manifest: &str, asset_name: &str) -> Option<String> {
    manifest.lines().find_map(|line| {
        let line = line.trim();
        let (hash, name) = line.split_once(char::is_whitespace)?;
        let name = name.trim().trim_start_matches('*');
        (name == asset_name && hash.len() == 64).then(|| hash.to_ascii_lowercase())
    })
}

/// Verify a file against an expected lowercase SHA-256, streaming in 64 KiB chunks.
pub fn verify_sha256(path: &Path, expected: &str) -> Result<(), String> {
    let mut file =
        std::fs::File::open(path).map_err(|e| format!("open {}: {e}", path.display()))?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|e| format!("read {}: {e}", path.display()))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    let actual = format!("{:x}", hasher.finalize());
    if actual == expected.to_ascii_lowercase() {
        Ok(())
    } else {
        Err(format!(
            "sha256 mismatch: expected {expected}, got {actual}"
        ))
    }
}

/// Reject payloads that are obviously not the expected installer format.
pub fn validate_payload(path: &Path) -> Result<(), String> {
    let mut file =
        std::fs::File::open(path).map_err(|e| format!("open {}: {e}", path.display()))?;
    let mut magic = [0u8; 4];
    file.read_exact(&mut magic)
        .map_err(|e| format!("read magic: {e}"))?;
    #[cfg(target_os = "windows")]
    if &magic[..2] != b"MZ" {
        return Err("downloaded file is not a Windows executable".into());
    }
    #[cfg(target_os = "macos")]
    {
        // UDIF/UDZO images end with a 512-byte trailer starting with "koly".
        use std::io::{Seek, SeekFrom};
        let length = file
            .metadata()
            .map_err(|e| format!("stat {}: {e}", path.display()))?
            .len();
        if length < 512 {
            return Err("downloaded file is too small to be a DMG".into());
        }
        let mut trailer = [0u8; 512];
        file.seek(SeekFrom::End(-512))
            .map_err(|e| format!("seek {}: {e}", path.display()))?;
        file.read_exact(&mut trailer)
            .map_err(|e| format!("read DMG trailer: {e}"))?;
        if &trailer[..4] != b"koly" {
            return Err("downloaded file is not a DMG image".into());
        }
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    if magic != [0x1f, 0x8b, 0x08, 0x00] && magic != [0x1f, 0x8b, 0x08, 0x08] {
        return Err("downloaded file is not a gzip archive".into());
    }
    Ok(())
}

/// Download `url` to `destination` while hashing; rejects on mismatch.
pub fn download_verified(
    url: &str,
    destination: &Path,
    expected_sha256: &str,
    progress: &mut dyn FnMut(u8),
) -> Result<(), String> {
    let response = ureq::get(url)
        .call()
        .map_err(|e| format!("download failed: {e}"))?;
    let total: Option<u64> = response
        .header("Content-Length")
        .and_then(|value| value.parse().ok());
    let part = destination.with_extension("part");
    if let Some(parent) = destination.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("create {}: {e}", parent.display()))?;
    }
    let mut file =
        std::fs::File::create(&part).map_err(|e| format!("create {}: {e}", part.display()))?;
    let mut hasher = Sha256::new();
    let mut reader = response.into_reader();
    let mut buffer = [0u8; 64 * 1024];
    let mut written = 0u64;
    loop {
        let read = reader
            .read(&mut buffer)
            .map_err(|e| format!("read stream: {e}"))?;
        if read == 0 {
            break;
        }
        use std::io::Write as _;
        file.write_all(&buffer[..read])
            .map_err(|e| format!("write {}: {e}", part.display()))?;
        hasher.update(&buffer[..read]);
        written += read as u64;
        if let Some(total) = total.filter(|total| *total > 0) {
            progress(((written.min(total) * 100) / total) as u8);
        }
    }
    let actual = format!("{:x}", hasher.finalize());
    if actual != expected_sha256.to_ascii_lowercase() {
        let _ = std::fs::remove_file(&part);
        return Err(format!(
            "sha256 mismatch: expected {expected_sha256}, got {actual}"
        ));
    }
    std::fs::rename(&part, destination)
        .map_err(|e| format!("finalize {}: {e}", destination.display()))?;
    progress(100);
    Ok(())
}

/// Launch the verified installer after this process exits.
pub fn launch_installer(installer: &Path, parent_pid: u32) -> Result<(), String> {
    let installer = installer.display().to_string();
    #[cfg(target_os = "windows")]
    {
        let script = format!(
            "Wait-Process -Id {parent_pid} -Timeout 60 -ErrorAction SilentlyContinue; Start-Process -FilePath '{installer}' -ArgumentList '/S'"
        );
        std::process::Command::new("powershell.exe")
            .args([
                "-NoProfile",
                "-ExecutionPolicy",
                "Bypass",
                "-Command",
                &script,
            ])
            .spawn()
            .map_err(|e| format!("spawn installer helper: {e}"))?;
    }
    #[cfg(target_os = "macos")]
    {
        let _ = parent_pid;
        std::process::Command::new("open")
            .arg(&installer)
            .spawn()
            .map_err(|e| format!("open dmg: {e}"))?;
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let _ = parent_pid;
        std::process::Command::new(&installer)
            .spawn()
            .map_err(|e| format!("spawn installer: {e}"))?;
    }
    Ok(())
}
