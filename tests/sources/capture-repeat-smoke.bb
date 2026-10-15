AppTitle "Hard Time repeated presentation regression"
For resolution=1 To 2
 If resolution=1 Then w=640 : h=480 Else w=800 : h=600
 Graphics w,h,32,2
 SetBuffer BackBuffer()
 AutoMidHandle False
 For frame=1 To 4
  ClsColor 0,0,0
  Cls
  Color frame*40,50,200
  Rect 10,10,120,80,1
  Color 255,255,255
  Text 10,120,"Repeated capture "+frame
  Flip
  wanted=(frame*40 Shl 16)+(50 Shl 8)+200
  got=ReadPixel(20,20,BackBuffer()) And $FFFFFF
  If got<>wanted Then RuntimeError "Frame changed after Flip"
  shot=CreateImage(w,h)
  GrabImage shot,0,0
  If SaveImage(shot,"capture.bmp")<>1 Then RuntimeError "Save failed"
  FreeImage shot
  shot=LoadImage("capture.bmp")
  got=ReadPixel(20,20,ImageBuffer(shot)) And $FFFFFF
  If got<>wanted Then RuntimeError "Capture did not retain the current frame"
  FreeImage shot
 Next
Next
f=WriteFile("capture-repeat-smoke-result.txt")
WriteLine f,"PASS repeated Flip capture save reload across two resolutions"
CloseFile f
End
