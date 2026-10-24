//! Native backdrop vibrancy for egui viewports.
//!
//! egui's `ViewportInfo` exposes no native window handle, so the Windows
//! implementation resolves the window by process id + title (legacy titles
//! are unique per surface) and re-applies the effect whenever the live HWND
//! changes. macOS/Linux native vibrancy is a recorded degradation for now
//! (egui translucent visuals still read as glass); the platform hooks live
//! behind `cfg` so a later task can attach them without touching call sites.

use std::collections::HashMap;

use volumectl::ui::{MaterialMode, SurfaceId};

/// Per-surface material choice for the native backdrop.
#[must_use]
pub fn material_for(surface: SurfaceId) -> MaterialMode {
    match surface {
        SurfaceId::Settings => MaterialMode::Opaque,
        SurfaceId::Mixer | SurfaceId::Overlay | SurfaceId::Help => MaterialMode::Translucent,
        SurfaceId::Tray => MaterialMode::Opaque,
    }
}

/// Window title used for both the viewport and the HWND lookup.
#[must_use]
pub fn title_for(surface: SurfaceId, language: volumectl::config::Language) -> String {
    use crate::i18n::{tr, Key};
    match surface {
        SurfaceId::Overlay => tr(Key::OverlayVolume, language, &[]).to_string(),
        SurfaceId::Mixer => tr(Key::MixerTitle, language, &[]).to_string(),
        SurfaceId::Settings => tr(Key::AppWindowSettings, language, &[]).to_string(),
        SurfaceId::Help => tr(Key::AppWindowHelp, language, &[]).to_string(),
        SurfaceId::Tray => tr(Key::AppName, language, &[]).to_string(),
    }
}

/// Cache of applied effects keyed by live HWND.
#[derive(Debug, Default)]
pub struct VibrancyState {
    applied: HashMap<isize, (SurfaceId, MaterialMode, bool)>,
    taskbar_hidden: std::collections::HashSet<isize>,
}

impl VibrancyState {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Apply the surface's native backdrop effect once per live window.
    ///
    /// The lookup is retried every frame until the window exists; effects are
    /// re-applied when the HWND changes (viewport closed/reopened).
    pub fn apply(
        &mut self,
        surface: SurfaceId,
        material: MaterialMode,
        is_dark: bool,
        language: volumectl::config::Language,
    ) {
        if matches!(surface, SurfaceId::Tray) {
            return;
        }
        self.apply_platform(surface, material, is_dark, language);
    }

    #[cfg(target_os = "windows")]
    fn apply_platform(
        &mut self,
        surface: SurfaceId,
        material: MaterialMode,
        is_dark: bool,
        language: volumectl::config::Language,
    ) {
        let Some(hwnd) = windows_impl::find_window_by_title(&title_for(surface, language)) else {
            return;
        };
        if self.applied.get(&hwnd) == Some(&(surface, material, is_dark)) {
            return;
        }
        if let Some((old_surface, _, _)) = self.applied.remove(&hwnd) {
            let _ = old_surface;
            windows_impl::clear(hwnd);
        }
        if windows_impl::apply(hwnd, surface, material, is_dark).is_ok() {
            self.applied.insert(hwnd, (surface, material, is_dark));
        }
        if matches!(surface, SurfaceId::Mixer | SurfaceId::Overlay)
            && self.taskbar_hidden.insert(hwnd)
        {
            windows_impl::hide_from_taskbar(hwnd);
        }
    }

    #[cfg(not(target_os = "windows"))]
    fn apply_platform(
        &mut self,
        surface: SurfaceId,
        _material: MaterialMode,
        _is_dark: bool,
        _language: volumectl::config::Language,
    ) {
        // Recorded degradation: the compositor effect is applied per window
        // once a native handle is reachable from egui.
        log::debug!("native vibrancy unavailable for {surface:?}; egui translucent visuals only");
    }
}

#[cfg(target_os = "windows")]
mod windows_impl {
    use std::num::NonZeroIsize;

    use raw_window_handle::{
        HandleError, HasWindowHandle, RawWindowHandle, Win32WindowHandle, WindowHandle,
    };
    use volumectl::ui::{MaterialMode, SurfaceId};
    use windows_sys::Win32::Foundation::{BOOL, HWND, LPARAM, TRUE};
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GetWindowTextW, GetWindowThreadProcessId, IsWindow,
    };

    struct OwnedHwnd(isize);

    impl HasWindowHandle for OwnedHwnd {
        fn window_handle(&self) -> Result<WindowHandle<'_>, HandleError> {
            let Some(hwnd) = NonZeroIsize::new(self.0) else {
                return Err(HandleError::Unavailable);
            };
            let mut handle = Win32WindowHandle::new(hwnd);
            handle.hinstance = None;
            let raw = RawWindowHandle::Win32(handle);
            // SAFETY: the HWND belongs to this process and stays alive while
            // the window is open; the borrow is scoped to the call.
            Ok(unsafe { WindowHandle::borrow_raw(raw) })
        }
    }

    struct Search<'a> {
        title: &'a [u16],
        found: Option<isize>,
    }

    unsafe extern "system" fn enum_proc(hwnd: HWND, lparam: LPARAM) -> BOOL {
        let search = unsafe { &mut *(lparam as *mut Search<'_>) };
        let mut pid = 0u32;
        unsafe {
            GetWindowThreadProcessId(hwnd, &mut pid);
        }
        if pid != std::process::id() {
            return TRUE;
        }
        let mut buffer = [0u16; 256];
        let len = unsafe { GetWindowTextW(hwnd, buffer.as_mut_ptr(), buffer.len() as i32) };
        if len <= 0 {
            return TRUE;
        }
        let len = len as usize;
        if buffer[..len] == *search.title {
            search.found = Some(hwnd);
            return 0;
        }
        TRUE
    }

    /// Find this process's window whose title matches exactly.
    pub fn find_window_by_title(title: &str) -> Option<isize> {
        let mut wide: Vec<u16> = title.encode_utf16().collect();
        wide.push(0);
        let mut search = Search {
            title: &wide[..wide.len() - 1],
            found: None,
        };
        // SAFETY: the callback only writes through the LPARAM we hand it and
        // returns TRUE until a match is found.
        unsafe {
            EnumWindows(Some(enum_proc), &mut search as *mut Search<'_> as LPARAM);
        }
        search.found.filter(|hwnd| unsafe { IsWindow(*hwnd) } != 0)
    }

    pub fn apply(
        hwnd: isize,
        surface: SurfaceId,
        material: MaterialMode,
        is_dark: bool,
    ) -> Result<(), window_vibrancy::Error> {
        let handle = OwnedHwnd(hwnd);
        match surface {
            SurfaceId::Settings => window_vibrancy::apply_mica(&handle, Some(is_dark)),
            SurfaceId::Mixer | SurfaceId::Overlay | SurfaceId::Help => {
                let color = if is_dark {
                    match material {
                        MaterialMode::Translucent => Some((15, 15, 18, 180)),
                        _ => Some((24, 24, 28, 220)),
                    }
                } else {
                    match material {
                        MaterialMode::Translucent => Some((250, 250, 255, 180)),
                        _ => Some((240, 240, 245, 220)),
                    }
                };
                window_vibrancy::apply_acrylic(&handle, color)
            }
            SurfaceId::Tray => Ok(()),
        }
    }

    /// Mark a window as a tool window so it never appears in the taskbar.
    pub fn hide_from_taskbar(hwnd: isize) {
        use windows_sys::Win32::UI::WindowsAndMessaging::{
            GetWindowLongW, SetWindowLongW, GWL_EXSTYLE, WS_EX_TOOLWINDOW,
        };
        // SAFETY: plain style read/write on a live HWND owned by this process.
        unsafe {
            let style = GetWindowLongW(hwnd, GWL_EXSTYLE);
            SetWindowLongW(hwnd, GWL_EXSTYLE, style | WS_EX_TOOLWINDOW as i32);
        }
    }

    pub fn clear(hwnd: isize) {
        let handle = OwnedHwnd(hwnd);
        let _ = window_vibrancy::clear_mica(&handle);
        let _ = window_vibrancy::clear_acrylic(&handle);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn surface_titles_are_unique() {
        let language = volumectl::config::Language::En;
        let titles = [
            title_for(SurfaceId::Mixer, language),
            title_for(SurfaceId::Settings, language),
            title_for(SurfaceId::Help, language),
            title_for(SurfaceId::Overlay, language),
        ];
        for (index, title) in titles.iter().enumerate() {
            assert!(titles[index + 1..].iter().all(|other| other != title));
        }
    }

    #[test]
    fn hud_surfaces_request_translucent_material() {
        assert_eq!(material_for(SurfaceId::Mixer), MaterialMode::Translucent);
        assert_eq!(material_for(SurfaceId::Overlay), MaterialMode::Translucent);
    }

    #[test]
    fn tray_has_no_vibrancy_state() {
        let mut state = VibrancyState::new();
        state.apply(
            SurfaceId::Tray,
            MaterialMode::Translucent,
            true,
            volumectl::config::Language::En,
        );
        assert!(state.applied.is_empty());
    }
}
