; NSIS hooks for VolumeControl.
; Ask a running instance to exit gracefully instead of letting the upstream
; Restart Manager fallback force-kill it.
;
; The bundled template defines its own `CheckIfAppIsRunning` macro in
; `utils.nsh`, so redefining it here fails to compile
; ("macro named ... already exists!"). The hooks below run immediately before
; that macro is inserted, and abort unless the app is gone, so the force-kill
; path (`RmForceShutdown`) is not reachable. Never force-kills or calls
; TerminateProcess.

; Requests a graceful exit through the Local\VolumeControl.Exit event, waits up
; to 15 seconds for the process owning the exact installed executable, and
; aborts the section when it is still running.
; NSIS expands `$` in strings, so PowerShell variables are written as `$$`.
!macro VolumeControl_StopApp id action
  DetailPrint "Requesting graceful shutdown of VolumeControl if running..."
  nsExec::ExecToLog 'powershell.exe -NoProfile -ExecutionPolicy Bypass -Command "$$exe = \"$INSTDIR\VolumeControl.exe\"; $$h = $$null; try { $$h = [System.Threading.EventWaitHandle]::OpenExisting(\"Local\VolumeControl.Exit\"); $$h.Set(); $$h.Dispose() } catch {}; foreach ($$p in @(Get-Process -Name VolumeControl -ErrorAction SilentlyContinue | Where-Object { try { $$_.Path -eq $$exe } catch { $$false } })) { $$null = $$p.WaitForExit(15000) }; $$still = @(Get-Process -Name VolumeControl -ErrorAction SilentlyContinue | Where-Object { try { $$_.Path -eq $$exe } catch { $$false } }); if ($$still.Count -gt 0) { exit 1 } else { exit 0 }"'
  Pop $0
  StrCmp $0 "0" volumecontrol_stop_ok_${id}
  DetailPrint "VolumeControl is still running. Please exit it from the tray icon before the ${action} and retry."
  SetErrorLevel 1
  Abort
  volumecontrol_stop_ok_${id}:
!macroend

!macro NSIS_HOOK_PREINSTALL
  !insertmacro VolumeControl_StopApp preinstall update
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  !insertmacro VolumeControl_StopApp preuninstall uninstall
!macroend
