//! Native backdrop vibrancy for egui viewports.
//!
//! egui's `ViewportInfo` exposes no native window handle, so the Windows
//! implementation resolves the window by process id + title (legacy titles
//! are unique per surface) and re-applies the effect whenever the live HWND
//! changes. macOS/Linux native vibrancy is a recorded degradation for now
//! (egui translucent visuals still read as glass); the platform hooks live
//! behind `cfg` so a later task can attach them without touching call sites.

#[cfg(target_os = "windows")]
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
/// Physical cursor position, used to place the tray menu at the pointer.
#[must_use]
pub fn cursor_position() -> Option<(f32, f32)> {
    #[cfg(target_os = "windows")]
    {
        use windows_sys::Win32::Foundation::POINT;
        use windows_sys::Win32::UI::WindowsAndMessaging::GetCursorPos;
        let mut point = POINT { x: 0, y: 0 };
        // SAFETY: GetCursorPos fills the provided POINT for this desktop.
        if unsafe { GetCursorPos(&mut point) } != 0 {
            return Some((point.x as f32, point.y as f32));
        }
    }
    None
}

/// Match native popup menus (tray menu, context menus) to the app theme.
pub fn set_system_menus_dark(dark: bool) {
    #[cfg(target_os = "windows")]
    windows_impl::set_system_menus_dark(dark);
    #[cfg(not(target_os = "windows"))]
    let _ = dark;
}

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
    #[cfg(target_os = "windows")]
    applied: HashMap<isize, (SurfaceId, MaterialMode, bool)>,
    #[cfg(target_os = "windows")]
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
        windows_impl::apply_icon(hwnd);
        windows_impl::round_corners(hwnd);
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

    /// Force uxtheme popup menus into the app's theme (Windows 11 22000+).
    ///
    /// `SetPreferredAppMode` (ordinal 135) and `FlushMenuThemes` (136) are the
    /// documented-by-usage entry points used by WinUI apps. Failure is silent:
    /// unsupported builds simply keep their native menu colors.
    pub fn set_system_menus_dark(dark: bool) {
        use windows_sys::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryW};
        type SetPreferredAppMode = unsafe extern "system" fn(i32) -> i32;
        type FlushMenuThemes = unsafe extern "system" fn();
        let name: Vec<u16> = "uxtheme.dll"
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        // SAFETY: uxtheme is a system DLL; ordinals 135/136 are stable since
        // Windows 11 22000 and the pointers are only called when present.
        unsafe {
            let lib = LoadLibraryW(name.as_ptr());
            if lib == 0 {
                return;
            }
            let set_mode = GetProcAddress(lib, 135 as *const u8).map(|p| {
                // SAFETY: ordinal 135 has this signature on supported builds.
                std::mem::transmute_copy::<_, SetPreferredAppMode>(&p)
            });
            let flush = GetProcAddress(lib, 136 as *const u8).map(|p| {
                // SAFETY: ordinal 136 takes no arguments.
                std::mem::transmute_copy::<_, FlushMenuThemes>(&p)
            });
            if let Some(set_mode) = set_mode {
                set_mode(if dark { 2 } else { 3 });
            }
            if let Some(flush) = flush {
                flush();
            }
        }
    }

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
        let tint = if is_dark {
            match material {
                MaterialMode::Transparent => Some((15, 15, 18, 90)),
                MaterialMode::Translucent | MaterialMode::LiquidGlass => Some((15, 15, 18, 180)),
                _ => Some((24, 24, 28, 220)),
            }
        } else {
            match material {
                MaterialMode::Transparent => Some((250, 250, 255, 90)),
                MaterialMode::Translucent | MaterialMode::LiquidGlass => Some((250, 250, 255, 180)),
                _ => Some((240, 240, 245, 220)),
            }
        };
        let effect = if surface == SurfaceId::Settings && material == MaterialMode::Auto {
            MaterialMode::Mica
        } else {
            material
        };
        match effect {
            MaterialMode::Opaque => {
                let _ = window_vibrancy::clear_acrylic(&handle);
                let _ = window_vibrancy::clear_mica(&handle);
                let _ = window_vibrancy::clear_blur(&handle);
                Ok(())
            }
            MaterialMode::Mica | MaterialMode::Auto if surface == SurfaceId::Settings => {
                window_vibrancy::apply_mica(&handle, Some(is_dark))
            }
            MaterialMode::Mica => window_vibrancy::apply_acrylic(&handle, tint),
            MaterialMode::Blur => window_vibrancy::apply_blur(&handle, tint),
            MaterialMode::Transparent => {
                let _ = window_vibrancy::clear_mica(&handle);
                let _ = window_vibrancy::clear_blur(&handle);
                window_vibrancy::apply_acrylic(&handle, tint)
            }
            // Windows has no Liquid Glass; acrylic with the glass tint is the
            // closest transparent backdrop until macOS 26 `NSGlassEffectView`.
            MaterialMode::Acrylic
            | MaterialMode::Translucent
            | MaterialMode::LiquidGlass
            | MaterialMode::Auto => window_vibrancy::apply_acrylic(&handle, tint),
        }
    }

    /// Ask DWM for rounded corners (Windows 11 22000+); no-op elsewhere.
    pub fn round_corners(hwnd: isize) {
        use windows_sys::Win32::Graphics::Dwm::DwmSetWindowAttribute;
        // DWMWA_WINDOW_CORNER_PREFERENCE = 33, DWMWCP_ROUND = 2.
        let preference: i32 = 2;
        // SAFETY: the attribute and value are documented for Windows 11; older
        // builds reject the call and keep square corners.
        unsafe {
            DwmSetWindowAttribute(
                hwnd,
                33,
                std::ptr::addr_of!(preference).cast(),
                std::mem::size_of::<i32>() as u32,
            );
        }
    }

    /// Install the packaged app icon on a top-level window.
    ///
    /// egui-winit sets the icon at creation, but Windows keeps the default for
    /// immediate child viewports; re-applying `WM_SETICON` fixes the taskbar.
    pub fn apply_icon(hwnd: isize) {
        use std::sync::OnceLock;
        use windows_sys::Win32::UI::WindowsAndMessaging::{
            LoadImageW, SendMessageW, ICON_BIG, ICON_SMALL, IMAGE_ICON, LR_LOADFROMFILE, WM_SETICON,
        };
        static ICONS: OnceLock<Option<(isize, isize)>> = OnceLock::new();
        let icons = ICONS.get_or_init(|| {
            let path = icon_file_path()?;
            let wide: Vec<u16> = path
                .to_string_lossy()
                .encode_utf16()
                .chain(std::iter::once(0))
                .collect();
            // SAFETY: loading a file-backed icon; the returned handles live for
            // the process lifetime (cached).
            let big = unsafe { LoadImageW(0, wide.as_ptr(), IMAGE_ICON, 0, 0, LR_LOADFROMFILE) };
            let small =
                unsafe { LoadImageW(0, wide.as_ptr(), IMAGE_ICON, 16, 16, LR_LOADFROMFILE) };
            (big != 0).then_some((big, small))
        });
        let Some((big, small)) = icons else {
            return;
        };
        // SAFETY: HWND belongs to this process; ICON_* are documented values.
        unsafe {
            SendMessageW(hwnd, WM_SETICON, ICON_BIG as usize, *big);
            if *small != 0 {
                SendMessageW(hwnd, WM_SETICON, ICON_SMALL as usize, *small);
            }
        }
    }

    /// Write a PNG-compressed .ico next to the app cache and return its path.
    fn icon_file_path() -> Option<std::path::PathBuf> {
        let png: &[u8] = include_bytes!("../../../src-tauri/icons/128x128.png");
        let mut ico = Vec::with_capacity(png.len() + 22);
        ico.extend_from_slice(&[0, 0, 1, 0, 1, 0]); // ICONDIR
        ico.push(128); // width
        ico.push(128); // height
        ico.push(0); // palette
        ico.push(0); // reserved
        ico.extend_from_slice(&1u16.to_le_bytes()); // planes
        ico.extend_from_slice(&32u16.to_le_bytes()); // bpp
        ico.extend_from_slice(&(png.len() as u32).to_le_bytes());
        ico.extend_from_slice(&22u32.to_le_bytes());
        ico.extend_from_slice(png);
        let root = std::env::var_os("LOCALAPPDATA")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(std::env::temp_dir);
        let dir = root.join("VolumeControl");
        std::fs::create_dir_all(&dir).ok()?;
        let path = dir.join("app.ico");
        if std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0) != ico.len() as u64 {
            std::fs::write(&path, &ico).ok()?;
        }
        Some(path)
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
