AppTitle "Hard Time native runtime smoke"
Graphics3D 320,240,32,2
Color 12,34,56
If ColorRed()<>12 Or ColorGreen()<>34 Or ColorBlue()<>56 Then RuntimeError "Color roundtrip"
f=WriteFile("runtime-smoke.bin")
WriteInt f,-2147483647-1
WriteInt f,-1
WriteInt f,0
WriteInt f,2147483647
WriteFloat f,1.25
WriteFloat f,-2.5
WriteString f,"Hard Time"
CloseFile f
f=ReadFile("runtime-smoke.bin")
If ReadInt(f)<>-2147483647-1 Then RuntimeError "ReadInt minimum"
If ReadInt(f)<>-1 Then RuntimeError "ReadInt negative"
If ReadInt(f)<>0 Then RuntimeError "ReadInt zero"
If ReadInt(f)<>2147483647 Then RuntimeError "ReadInt maximum"
If ReadFloat(f)<>1.25 Then RuntimeError "ReadFloat positive"
If ReadFloat(f)<>-2.5 Then RuntimeError "ReadFloat negative"
If ReadString(f)<>"Hard Time" Then RuntimeError "ReadString"
CloseFile f
timer=CreateTimer(30)
start=MilliSecs()
ticks=0
For i=1 To 5
 ticks=ticks+WaitTimer(timer)
Next
Delay 120
catchup=WaitTimer(timer)
elapsed=MilliSecs()-start
FreeTimer timer
f=WriteFile("runtime-smoke-result.txt")
WriteLine f,"Ticks="+ticks
WriteLine f,"Catchup="+catchup
WriteLine f,"Elapsed="+elapsed
CloseFile f
If ticks<5 Or catchup<3 Or elapsed<250 Or elapsed>1000 Then RuntimeError "Timer cadence"
End
