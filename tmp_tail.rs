        main_context.with_thread_default(|| {
            log::info!("linux host running on the GTK main loop");
            eprintln!("DBG is_owner={} entering run", main_context.is_owner());
            timeout_add_local(Duration::from_millis(150), move || {
                eprintln!("DBG TICK");
                {
                    let mut c = ctx_rc.borrow_mut();
                    while let Ok(action) = rx_rc.try_recv() {
                        apply_action(&mut c, &action);
                    }
                }
                refresh_from_audio(&mut ctx_rc.borrow_mut());
                if ctx_rc.borrow().quit_requested {
                    main_loop_quit.quit();
                    return ControlFlow::Break;
                }
                ControlFlow::Continue
            });
            main_loop.run();
            eprintln!("DBG run returned is_owner={}", main_context.is_owner());
        });
        Ok(())
    }
}

/// Linux application entry point (GTK renderer).
///
/// Initializes GTK; if no display session is available the error is returned
/// so the caller can fall back to the CLI.
#[cfg(all(target_os = "linux", feature = "gtk-renderer"))]
pub fn run() -> Result<(), String> {
    gtk_host::run()
}
