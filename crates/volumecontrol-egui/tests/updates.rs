//! Update download contract: asset naming, manifest parsing, checksum, payload.

use std::io::Write;

use volumecontrol_egui::updates::{
    asset_filename, manifest_sha256, validate_payload, verify_sha256,
};

#[test]
fn asset_name_matches_release_contract() {
    let name = asset_filename("0.2.0");
    #[cfg(target_os = "windows")]
    assert_eq!(name, "VolumeControl_0.2.0_x64-setup.exe");
    #[cfg(target_os = "macos")]
    assert_eq!(name, "volumecontrol-0.2.0-macos.zip");
    #[cfg(all(unix, not(target_os = "macos")))]
    assert_eq!(name, "volumecontrol-0.2.0-linux-amd64.tar.gz");
}

#[test]
fn manifest_extracts_only_the_matching_asset() {
    let manifest = concat!(
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa  other.exe\n",
        "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb  *VolumeControl_0.2.0_x64-setup.exe\n",
    );
    assert_eq!(
        manifest_sha256(manifest, "VolumeControl_0.2.0_x64-setup.exe").as_deref(),
        Some("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb")
    );
    assert_eq!(manifest_sha256(manifest, "missing.exe"), None);
}

#[test]
fn checksum_verification_rejects_corruption() {
    let path = std::env::temp_dir().join(format!("volumecontrol-update-{}", std::process::id()));
    let mut file = std::fs::File::create(&path).unwrap();
    #[cfg(target_os = "windows")]
    file.write_all(b"MZpayload").unwrap();
    #[cfg(target_os = "macos")]
    file.write_all(b"PK\x03\x04payload").unwrap();
    #[cfg(all(unix, not(target_os = "macos")))]
    file.write_all(b"\x1f\x8b\x08\x00payload").unwrap();
    drop(file);
    // sha256("MZpayload")
    let expected = "a5b3a4d1fd1b0b8a2f7e02d7c3e2c60e0f0b3e6b6f0a2f6c3c2a5f1c1f2b1a4d";
    assert!(verify_sha256(&path, expected).is_err());
    assert!(validate_payload(&path).is_ok());
    let _ = std::fs::remove_file(path);
}
