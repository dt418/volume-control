; NSIS hooks for VolumeControl.
; Overrides the default force-kill behavior with graceful exit requests.
; Never force-kills or calls TerminateProcess.


!macro NSIS_HOOK_PREINSTALL
  DetailPrint "Requesting graceful shutdown of VolumeControl if running..."
  nsExec::ExecToLog 'powershell.exe -NoProfile -ExecutionPolicy Bypass -Command "$h = [System.Threading.EventWaitHandle]::OpenExisting(\"Local\\VolumeControl.Exit\"); if ($h) { $h.Set(); $h.Dispose() }; $p = Get-Process VolumeControl -ErrorAction SilentlyContinue; if ($p) { $p.WaitForExit(15000) }; if (Get-Process VolumeControl -ErrorAction SilentlyContinue) { exit 1 }"'
  Pop $0
  StrCmp $0 "0" success
  DetailPrint "VolumeControl is still running. Please close it via the tray icon before updating."
  Abort
success:
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  DetailPrint "Requesting graceful shutdown of VolumeControl if running..."
  nsExec::ExecToLog 'powershell.exe -NoProfile -ExecutionPolicy Bypass -Command "$h = [System.Threading.EventWaitHandle]::OpenExisting(\"Local\\VolumeControl.Exit\"); if ($h) { $h.Set(); $h.Dispose() }; $p = Get-Process VolumeControl -ErrorAction SilentlyContinue; if ($p) { $p.WaitForExit(15000) }; if (Get-Process VolumeControl -ErrorAction SilentlyContinue) { exit 1 }"'
  Pop $0
  StrCmp $0 "0" success
  DetailPrint "VolumeControl is still running. Please close it via the tray icon before uninstalling."
  Abort
success:
!macroend
