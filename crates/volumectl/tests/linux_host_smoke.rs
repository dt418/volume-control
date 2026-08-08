//! Main-thread GTK Linux host smoke test.
//!
//! Xvfb proves only GTK/X11 renderer wiring. A real Wayland compositor is a
//! separate runtime requirement and is never inferred from this test.

fn main() {
    #[cfg(all(target_os = "linux", feature = "gtk-renderer"))]
    {
        run_smoke();
        eprintln!("linux host smoke OK (X11/GTK only)");
    }
}

#[cfg(all(target_os = "linux", feature = "gtk-renderer"))]
fn run_smoke() {
    if let Err(error) = volumectl_lib::linux_app::run_smoke() {
        panic!("Linux host smoke failed: {error}");
    }
}
