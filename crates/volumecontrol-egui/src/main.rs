//! Egui/eframe runtime host for VolumeControl.
//!
//! Usage: `volumecontrol-host [--smoke] [--smoke-ms=N] [--show-all]
//! [--surface=<mixer|settings|help|overlay>] [--debug-ui]`

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use volumecontrol_egui::app::{builder_for, LaunchOptions, VolumeControlApp};
use volumecontrol_egui::cli::{self, Verb};
use volumecontrol_egui::placement::WorkArea;
use volumecontrol_egui::settings::model::Tab;
use volumecontrol_egui::single_instance::{self, Launch};
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
            "--overlay-demo" => options.overlay_demo = true,
            "--debug-ui" => options.debug_ui = true,
            "--background" => options.background = true,
            _ => {
                if let Some(ms) = arg
                    .strip_prefix("--smoke-ms=")
                    .and_then(|value| value.parse::<u64>().ok())
                {
                    options.smoke = true;
                    options.smoke_ms = ms;
                } else if let Some(tab) = arg.strip_prefix("--settings-tab=") {
                    options.settings_tab = match tab.to_lowercase().as_str() {
                        "general" => Some(Tab::General),
                        "hotkeys" => Some(Tab::Hotkeys),
                        "appearance" => Some(Tab::Appearance),
                        "blacklist" => Some(Tab::Blacklist),
                        "feedback" => Some(Tab::Feedback),
                        "storage" => Some(Tab::Storage),
                        other => {
                            log::warn!("unknown settings tab {other:?}");
                            None
                        }
                    };
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

    let raw_args: Vec<String> = std::env::args().skip(1).collect();
    if let Some(verb) = cli::parse(&raw_args) {
        let (config, notice) = config::load_with_notice();
        let (sink, _events) = EguiSink::new(eframe::egui::Context::default());
        let audio =
            audio::default_backend().map_err(|error| format!("audio unavailable: {error}"))?;
        let mut core =
            AppCore::new_with_notice(audio, config.clone(), config.modifier, sink, notice)
                .map_err(|error| format!("core init failed: {error}"))?;
        if verb == Verb::Get {
            println!("{}", cli::render_get_core(&core));
            return Ok(());
        }
        cli::apply(&mut core, verb)?;
        return Ok(());
    }

    let guard = match single_instance::acquire() {
        Launch::Primary(guard) => guard,
        Launch::Secondary => {
            // Ask the resident primary to reveal its Mixer, then exit.
            let _ = single_instance::signal_show_mixer();
            return Ok(());
        }
    };

    let options = parse_options();
    let (config, notice) = config::load_with_notice();
    let area = WorkArea::new(0.0, 0.0, 1600.0, 900.0);
    #[cfg_attr(not(target_os = "windows"), allow(unused_mut))]
    let mut native_options = eframe::NativeOptions {
        viewport: builder_for(SurfaceId::Mixer, 1.0, &area, config.language),
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

    let smoke_verified = Arc::new(AtomicBool::new(false));
    let smoke_flag = smoke_verified.clone();
    let smoke_requested = options.smoke;

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
            let mut app = VolumeControlApp::new(core, events, options, smoke_flag);
            app.attach_primary(guard);
            Ok(Box::new(app))
        }),
    )
    .map_err(|error| format!("eframe failed: {error}"))?;
    if smoke_requested && !smoke_verified.load(Ordering::Acquire) {
        return Err("smoke failed: not all surfaces were presented".into());
    }
    Ok(())
}
