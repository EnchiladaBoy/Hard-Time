AppTitle "Hard Time font smoke"
Graphics3D 1024,768,32,2
SetBuffer BackBuffer()
out=WriteFile("font-smoke-result.txt")
For iteration=1 To 10
 f=LoadFont("Comfortaa",96)
 If f=0 Then RuntimeError "Font load failed"
 SetFont f
 chars$=""
 For code=32 To 126
  chars$=chars$+Chr(code)
 Next
 Cls
 For line=0 To 9
  Text 10,line*76,Mid(chars$,line*10+1,10)
 Next
 Flip
 WriteLine out,"Iteration="+iteration+" Width="+StringWidth(chars$)+" Height="+FontHeight()
 FreeFont f
Next
WriteLine out,"PASS"
CloseFile out
Delay 500
End
