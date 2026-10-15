AppTitle "Hard Time close event regression"
AppCloseEvents True
Graphics3D 320,240,32,2
timer=CreateTimer(30)
If AppCloseRequested() Then RuntimeError "Premature close event"
f=WriteFile("ready.txt")
WriteLine f,"READY"
CloseFile f
While Not AppCloseRequested()
 WaitTimer timer
 Cls
 Flip
Wend
; This write must run after the native window receives WM_CLOSE.
f=WriteFile("window-close-smoke-result.txt")
WriteLine f,"PASS close event reached application cleanup"
CloseFile f
FreeTimer timer
End
