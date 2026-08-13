use tauri::{Manager, State};

use window_manager::{SurfaceId, WindowManager};

mod window_manager;

pub fn builder() -> tauri::Builder<tauri::Wry> {
    tauri::Builder::default().invoke_handler(tauri::generate_handler![open_surface, close_surface])
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() -> tauri::Result<()> {
    tauri::Builder::default()
        .setup(|app| {
            let handle = app.handle().clone();
            app.manage(WindowManager::new(handle));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![open_surface, close_surface])
        .run(tauri::generate_context!())
}

#[tauri::command]
fn open_surface(window_manager: State<'_, WindowManager>, surface: String) -> Result<(), String> {
    let surface = SurfaceId::from_label(&surface).ok_or("unknown surface")?;
    window_manager.open(surface)
}

#[tauri::command]
fn close_surface(window_manager: State<'_, WindowManager>, surface: String) -> Result<(), String> {
    let surface = SurfaceId::from_label(&surface).ok_or("unknown surface")?;
    window_manager.close(surface)
}
