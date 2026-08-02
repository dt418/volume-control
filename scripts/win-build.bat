@echo off
REM VolumeControl Windows build wrapper.
REM Calls vcvars64.bat to set the MSVC environment (link.exe, LIB, INCLUDE),
REM then runs the given cargo command (default: build).
REM Usage: scripts\win-build.bat [cargo args...]

setlocal
call "C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\VC\Auxiliary\Build\vcvars64.bat" >nul 2>&1

set CARGO=%USERPROFILE%\.cargo\bin\cargo.exe
if "%1"=="" (
    %CARGO% build
) else (
    %CARGO% %*
)
endlocal
