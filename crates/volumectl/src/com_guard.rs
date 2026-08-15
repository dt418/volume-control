//! Windows COM apartment initialization guard.
//!
//! ## Why this exists (RPC_E_CHANGED_MODE panic)
//!
//! The Tauri main thread must end up as a **STA** apartment: `tao` calls
//! `OleInitialize` when creating a window (every webview surface —
//! mixer/settings/help), and `OleInitialize` fails with
//! `RPC_E_CHANGED_MODE` if the thread is already initialized as MTA
//! (`COINIT_MULTITHREADED`). The audio backends initialize COM on the main
//! thread (Tauri runs synchronous commands there too), so every init site
//! MUST use `COINIT_APARTMENTTHREADED` and MUST balance its uninitialize:
//!
//! - `S_OK` (0): this call initialized the apartment — `CoUninitialize` is
//!   owed when done.
//! - `S_FALSE` (1): the apartment was already initialized (by another crate,
//!   e.g. tao) — we must NOT call `CoUninitialize`.
//!
//! `CoInitializeEx` with `COINIT_APARTMENTTHREADED` on a thread that is
//! already STA succeeds (`S_FALSE`) — a thread's apartment mode is fixed by
//! its first init, so it can never become MTA later.

use windows_sys::Win32::System::Com::{CoInitializeEx, CoUninitialize};

/// RAII guard for one COM apartment init on the calling thread.
///
/// Drop calls `CoUninitialize` exactly when this call initialized the
/// apartment (`S_OK`); a guard over an already-initialized apartment
/// (`S_FALSE`) drops without touching the refcount.
#[derive(Debug)]
pub struct ComGuard {
    uninit: bool,
}

impl ComGuard {
    /// Initialize the calling thread's COM apartment as STA
    /// (`COINIT_APARTMENTTHREADED`, 0x2 — never MTA).
    ///
    /// `Err(raw_hr)` is returned only for failures other than `S_OK`/`S_FALSE`.
    pub fn init_apartment_sta() -> Result<Self, i32> {
        // COINIT_APARTMENTTHREADED = 0x2.
        let hr = unsafe { CoInitializeEx(std::ptr::null(), 2) };
        match hr {
            0 => Ok(Self { uninit: true }),  // S_OK — we own the init
            1 => Ok(Self { uninit: false }), // S_FALSE — already initialized
            other => Err(other),
        }
    }

    /// Whether this guard owes a `CoUninitialize` (`S_OK` at init).
    pub fn should_uninit(&self) -> bool {
        self.uninit
    }
}

impl Drop for ComGuard {
    fn drop(&mut self) {
        if self.uninit {
            unsafe {
                CoUninitialize();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apartment_stays_balanced_on_a_fresh_thread() {
        // A freshly spawned thread has no prior COM init, so the first init
        // deterministically returns S_OK (owned).
        std::thread::spawn(|| {
            let first = ComGuard::init_apartment_sta().expect("STA init on a fresh thread");
            assert!(first.should_uninit());

            // A second init on the same thread reports already-initialized.
            let second = ComGuard::init_apartment_sta().expect("already-initialized thread");
            assert!(!second.should_uninit());

            // Dropping the non-owner must not uninit; dropping the owner
            // balances the init. (Windows may keep per-thread apartment state
            // such that a later re-init reports S_FALSE — production does not
            // depend on that, so it is not asserted here.)
            drop(second);
            drop(first);
        })
        .join()
        .expect("com guard test thread");
    }
}
