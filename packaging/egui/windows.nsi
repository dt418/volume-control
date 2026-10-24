; VolumeControl — native egui host installer (per-user, no admin).
; Built by scripts/package-egui.ps1 with:
;   makensis /DVERSION=x.y.z /DSRC=... /DOUT=... /DICON=... packaging\egui\windows.nsi

Unicode true
!include "MUI2.nsh"
!include "FileFunc.nsh"

!ifndef VERSION
  !define VERSION "0.0.0"
!endif
!ifndef SRC
  !error "SRC must point at the directory holding volumecontrol-host.exe"
!endif
!ifndef OUT
  !define OUT "VolumeControl-${VERSION}-x64-setup.exe"
!endif
!ifndef ICON
  !define ICON "..\..\src-tauri\icons\icon.ico"
!endif

Name "Volume Control"
OutFile "${OUT}"
InstallDir "$LOCALAPPDATA\Programs\VolumeControl"
InstallDirRegKey HKCU "Software\VolumeControl" "InstallDir"
RequestExecutionLevel user
SetCompressor /SOLID lzma
BrandingText "Volume Control ${VERSION}"

!define MUI_ICON "${ICON}"
!define MUI_UNICON "${ICON}"
!define MUI_ABORTWARNING

!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_PAGE_FINISH
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES
!insertmacro MUI_LANGUAGE "English"
!insertmacro MUI_LANGUAGE "Vietnamese"

Section "Volume Control" SEC_MAIN
  SetOutPath "$INSTDIR"
  File "${SRC}\volumecontrol-host.exe"

  CreateDirectory "$SMPROGRAMS\Volume Control"
  CreateShortcut "$SMPROGRAMS\Volume Control\Volume Control.lnk" "$INSTDIR\volumecontrol-host.exe"
  CreateShortcut "$SMPROGRAMS\Volume Control\Uninstall Volume Control.lnk" "$INSTDIR\uninstall.exe"
  CreateShortcut "$DESKTOP\Volume Control.lnk" "$INSTDIR\volumecontrol-host.exe"

  WriteRegStr HKCU "Software\VolumeControl" "InstallDir" "$INSTDIR"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\VolumeControl" \
    "DisplayName" "Volume Control"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\VolumeControl" \
    "DisplayVersion" "${VERSION}"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\VolumeControl" \
    "DisplayIcon" "$INSTDIR\volumecontrol-host.exe"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\VolumeControl" \
    "Publisher" "Volume Control"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\VolumeControl" \
    "UninstallString" '"$INSTDIR\uninstall.exe"'
  WriteRegDWORD HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\VolumeControl" \
    "NoModify" 1
  WriteRegDWORD HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\VolumeControl" \
    "NoRepair" 1
  WriteUninstaller "$INSTDIR\uninstall.exe"
SectionEnd

Section "Uninstall"
  Delete "$INSTDIR\volumecontrol-host.exe"
  Delete "$INSTDIR\uninstall.exe"
  RMDir "$INSTDIR"
  Delete "$SMPROGRAMS\Volume Control\Volume Control.lnk"
  Delete "$SMPROGRAMS\Volume Control\Uninstall Volume Control.lnk"
  RMDir "$SMPROGRAMS\Volume Control"
  Delete "$DESKTOP\Volume Control.lnk"
  DeleteRegKey HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\VolumeControl"
  DeleteRegKey HKCU "Software\VolumeControl"
SectionEnd
