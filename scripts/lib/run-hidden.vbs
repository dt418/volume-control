Set sh = CreateObject("WScript.Shell")
' Launches a command with window style 0 (hidden) from a GUI-subsystem host.
' wscript.exe has no console attached, and the child is created with
' STARTF_USESHOWWINDOW/SW_HIDE, so unlike `powershell -WindowStyle Hidden` the
' console window is never shown even for a single frame before PowerShell
' parses its arguments.
If WScript.Arguments.Count < 1 Then WScript.Quit 2
cmd = ""
For i = 0 To WScript.Arguments.Count - 1
  cmd = cmd & """" & WScript.Arguments(i) & """ "
Next
sh.Run cmd, 0, False
