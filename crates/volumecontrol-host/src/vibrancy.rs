//! Cross-platform native window vibrancy and backdrop blur integration.
//!
//! Applies OS-native compositor effects:
//! - Windows: Mica on Windows 11, Acrylic / Accent policy on Windows 10/11.
//! - macOS: NSVisualEffectView with HudWindow / AppearanceBased material.
//! - Linux: Compositor blur hints (X11 atom _KDE_NET_WM_BLUR_BEHIND_REGION, Wayland namespace).

use volumectl::ui::{MaterialMode, SurfaceId};
use winit::window::Window;

/// Apply appropriate platform vibrancy / backdrop blur for the given surface.
pub fn apply_vibrancy_for_surface(
    window: &Window,
    surface: SurfaceId,
    material: MaterialMode,
    is_dark: bool,
) {
    if material == MaterialMode::Opaque {
        clear_vibrancy(window);
        return;
    }

    #[cfg(target_os = "windows")]
    apply_windows_vibrancy(window, surface, material, is_dark);

    #[cfg(target_os = "macos")]
    apply_macos_vibrancy(window, surface);

    #[cfg(target_os = "linux")]
    apply_linux_vibrancy(window, surface);
}

/// Clear vibrancy effects, restoring normal window composition.
pub fn clear_vibrancy(window: &Window) {
    #[cfg(target_os = "windows")]
    {
        let _ = window_vibrancy::clear_mica(window);
        let _ = window_vibrancy::clear_acrylic(window);
        let _ = window_vibrancy::clear_blur(window);
    }
}

#[cfg(target_os = "windows")]
fn apply_windows_vibrancy(
    window: &Window,
    surface: SurfaceId,
    material: MaterialMode,
    is_dark: bool,
) {
    match surface {
        SurfaceId::Settings => {
            // Windows 11 Mica effect is designed for windowed dialogs and settings surfaces
            if window_vibrancy::apply_mica(window, Some(is_dark)).is_err() {
                // Fallback to Acrylic if Mica is not supported (Windows 10)
                let color = if is_dark {
                    Some((20, 20, 24, 210))
                } else {
                    Some((245, 245, 250, 210))
                };
                let _ = window_vibrancy::apply_acrylic(window, color);
            }
        }
        SurfaceId::Mixer | SurfaceId::Overlay | SurfaceId::Help => {
            // Floating HUD cards look best with Acrylic blur
            let color = match material {
                MaterialMode::Translucent => {
                    if is_dark {
                        Some((15, 15, 18, 180))
                    } else {
                        Some((250, 250, 255, 180))
                    }
                }
                _ => {
                    if is_dark {
                        Some((24, 24, 28, 220))
                    } else {
                        Some((240, 240, 245, 220))
                    }
                }
            };
            if window_vibrancy::apply_acrylic(window, color).is_err() {
                let _ = window_vibrancy::apply_blur(window, color);
            }
        }
        SurfaceId::Tray => {}
    }
}

#[cfg(target_os = "macos")]
fn apply_macos_vibrancy(window: &Window, surface: SurfaceId) {
    use window_vibrancy::{apply_vibrancy, NSVisualEffectMaterial, NSVisualEffectState};

    let material = match surface {
        SurfaceId::Overlay | SurfaceId::Mixer => NSVisualEffectMaterial::HudWindow,
        SurfaceId::Settings => NSVisualEffectMaterial::Sidebar,
        SurfaceId::Help => NSVisualEffectMaterial::Popover,
        SurfaceId::Tray => return,
    };

    let _ = apply_vibrancy(window, material, Some(NSVisualEffectState::Active), None);
}

#[cfg(target_os = "linux")]
fn apply_linux_vibrancy(window: &Window, _surface: SurfaceId) {
    apply_x11_blur_behind(window);
}

/// Ask the X11 compositor (KWin, Picom) to blur behind the whole window.
///
/// Sets `_KDE_NET_WM_BLUR_BEHIND_REGION` to an empty CARDINAL region, which
/// both KWin and Picom interpret as "blur the entire window". Compositors
/// without blur support simply ignore the property, so failures are soft.
#[cfg(target_os = "linux")]
fn apply_x11_blur_behind(window: &Window) {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use x11rb::connection::Connection;
    use x11rb::protocol::xproto::{AtomEnum, ConnectionExt, PropMode};
    use x11rb::wrapper::ConnectionExt as ConnectionExtWrapper;

    // winit 0.30's public `WindowExtX11` is an empty marker trait; the X11
    // window id is reached through raw-window-handle instead (rwh_06).
    let xid: u32 = match window.window_handle() {
        Ok(handle) => match handle.as_raw() {
            RawWindowHandle::Xlib(h) => h.window as u32,
            RawWindowHandle::Xcb(h) => h.window.get(),
            _ => {
                // Wayland session: protocol blur is owned by compositor window
                // rules (winit 0.30 does not expose wl_display/wl_surface for a
                // protocol request from inside the app).
                log::debug!(
                    "backdrop blur on Wayland follows compositor window rules for 'volumecontrol-host'"
                );
                return;
            }
        },
        Err(error) => {
            log::debug!("X11 blur hint skipped: no raw window handle: {error}");
            return;
        }
    };
    let Ok((conn, _screen)) = x11rb::connect(None) else {
        log::debug!("X11 blur hint skipped: cannot connect to the display");
        return;
    };
    let atom = match conn.intern_atom(false, b"_KDE_NET_WM_BLUR_BEHIND_REGION") {
        Ok(cookie) => match cookie.reply() {
            Ok(reply) => reply.atom,
            Err(error) => {
                log::debug!("X11 blur hint skipped: intern_atom reply failed: {error}");
                return;
            }
        },
        Err(error) => {
            log::debug!("X11 blur hint skipped: intern_atom failed: {error}");
            return;
        }
    };
    if let Err(error) = conn.change_property32(
        PropMode::REPLACE,
        xid,
        atom,
        AtomEnum::CARDINAL,
        &[] as &[u32],
    ) {
        log::debug!("X11 blur hint skipped: change_property failed: {error}");
        return;
    }
    if let Err(error) = conn.flush() {
        log::debug!("X11 blur hint skipped: flush failed: {error}");
        return;
    }
    log::info!("X11 backdrop blur hint set for window {xid}");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vibrancy_api_compiles() {
        assert_eq!(MaterialMode::Opaque, MaterialMode::Opaque);
    }
}
