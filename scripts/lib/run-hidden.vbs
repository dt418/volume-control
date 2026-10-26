Set sh = CreateObject("WScript.Shell")
' Launches a command with window style 0 (hidden) from a GUI-subsystem host.
' wscript.exe has no console attached, and the child is created with
' STARTF_USESHOWWINDOW/SW_HIDE, so unlike `powershell -WindowStyle Hidden` the
' console window is never shown even for a single frame before PowerShell
' parses its arguments.
'
' The final argument (True) waits for the child. Without it wscript exits
' immediately, Task Scheduler considers the instance finished, and ExecutionTimeLimit
' and MultipleInstances=IgnoreNew stop governing the PowerShell process that is
' still running.
'
' If WScript is killed (for example by the task's execution time limit) the
' child keeps its own lifetime; the checker is written to exit on its own.
If WScript.Arguments.Count < 1 Then WScript.Quit 2
cmd = ""
For i = 0 To WScript.Arguments.Count - 1
  cmd = cmd & """" & WScript.Arguments(i) & """ "
Next
sh.Run cmd, 0, True
