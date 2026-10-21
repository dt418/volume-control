; Stop a running instance before replacing its executable. Without this, an
; update leaves the old process holding the file and the new binary cannot
; replace it; clicking the shortcut then has no listener for the new handoff.
!macro NSIS_HOOK_PREINSTALL
  DetailPrint "Closing any running VolumeControl instance..."
  nsExec::Exec '"$SYSDIR\taskkill.exe" /IM VolumeControl.exe /F'
  Pop $0
!macroend
