// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    volumecontrol_tauri_lib::run().expect("VolumeControl Tauri host failed to start");
}
