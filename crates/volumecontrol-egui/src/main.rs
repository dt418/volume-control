//! Egui/eframe runtime host for VolumeControl.
//!
//! Usage: `volumecontrol-egui [--smoke] [--smoke-ms=N] [--show-all]
//! [--surface=<mixer|settings|help|overlay>]`

use volumecontrol_egui::app::{builder_for, LaunchOptions, VolumeControlApp};
use volumecontrol_egui::placement::WorkArea;
use volumecontrol_egui::sink::EguiSink;

use volumectl::audio;
use volumectl::config;
use volumectl::host_core::AppCore;
use volumectl::ui::SurfaceId;

fn parse_options() -> LaunchOptions {
    let mut options = LaunchOptions::default();
    for arg in std::env::args().skip(1) {
        match arg.as_str() {
            "--smoke" => options.smoke = true,
            "--show-all" => options.show_all = true,
            _ => {
                if let Some(ms) = arg
                    .strip_prefix("--smoke-ms=")
                    .and_then(|value| value.parse::<u64>().ok())
                {
                    options.smoke = true;
                    options.smoke_ms = ms;
                } else if let Some(surface) = arg.strip_prefix("--surface=") {
                    options.target_surface = match surface.to_lowercase().as_str() {
                        "mixer" => Some(SurfaceId::Mixer),
                        "settings" => Some(SurfaceId::Settings),
                        "help" => Some(SurfaceId::Help),
                        "overlay" => Some(SurfaceId::Overlay),
                        other => {
                            log::warn!(
                                "unknown surface {other:?}; expected mixer|settings|help|overlay"
                            );
                            None
                        }
                    };
                }
            }
        }
    }
    options
}

fn main() -> Result<(), String> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .try_init()
        .ok();

    let options = parse_options();
    let (config, notice) = config::load_with_notice();
    let area = WorkArea::new(0.0, 0.0, 1600.0, 900.0);
    let mut native_options = eframe::NativeOptions {
        viewport: builder_for(SurfaceId::Mixer, 1.0, &area),
        persist_window: false,
        centered: false,
        ..Default::default()
    };
    // The Vulkan path crashes on this Intel iGPU; DX12 is the supported
    // Windows backend (egui-wgpu's default is backend-agnostic).
    #[cfg(target_os = "windows")]
    if let eframe::egui_wgpu::WgpuSetup::CreateNew(setup) =
        &mut native_options.wgpu_options.wgpu_setup
    {
        setup.instance_descriptor.backends = eframe::wgpu::Backends::DX12;
    }

    eframe::run_native(
        "VolumeControl",
        native_options,
        Box::new(move |cc| {
            volumecontrol_egui::fonts::install(&cc.egui_ctx);
            let (sink, events) = EguiSink::new(cc.egui_ctx.clone());
            let audio = match audio::default_backend() {
                Ok(backend) => backend,
                Err(error) => {
                    log::warn!("audio backend unavailable: {error}");
                    Box::new(audio::UnavailableAudio::new(error.to_string()))
                }
            };
            let core =
                AppCore::new_with_notice(audio, config.clone(), config.modifier, sink, notice)
                    .map_err(|error| -> Box<dyn std::error::Error + Send + Sync> {
                        error.into()
                    })?;
            let app = VolumeControlApp::new(core, events, options);
            Ok(Box::new(app))
        }),
    )
    .map_err(|error| format!("eframe failed: {error}"))
}
