Graphics 320,240,32,2
SetBuffer BackBuffer()
ClsColor 0,0,255
Cls
Color 255,0,0
Rect 0,0,160,120,1
Color 0,255,0
Rect 160,120,160,120,1
CheckPixel(10,10,$FF0000,BackBuffer(),"backbuffer red")
CheckPixel(200,200,$00FF00,BackBuffer(),"backbuffer green")
tiny=CreateImage(8,8):GrabImage tiny,0,0
Color 255,255,255:Rect 300,220,10,10,1
CheckPixel(305,225,$FFFFFF,BackBuffer(),"small GrabImage preserves viewport")
Color 0,255,0:Rect 300,220,10,10,1
Flip
CheckPixel(10,10,$FF0000,BackBuffer(),"post Flip backbuffer")
i=CreateImage(320,240)
GrabImage i,0,0
If SaveImage(i,"graphics-grab.bmp")<>1 Then RuntimeError "SaveImage failed"
ResizeImage i,160,120
CheckPixel(10,10,$FF0000,ImageBuffer(i),"resized red")
CheckPixel(120,100,$00FF00,ImageBuffer(i),"resized green")
If SaveImage(i,"graphics-resized.bmp")<>1 Then RuntimeError "Save resized failed"
j=LoadImage("graphics-resized.bmp")
CheckPixel(10,10,$FF0000,ImageBuffer(j),"reloaded red")
CheckPixel(120,100,$00FF00,ImageBuffer(j),"reloaded green")
ClsColor 0,0,0:Cls
DrawImage j,0,0
CheckPixel(10,10,$FF0000,BackBuffer(),"draw loaded red")
s=CreateImage(320,240):GrabImage s,0,0:SaveImage(s,"graphics-drawn.bmp")
CheckPixel(120,100,$00FF00,BackBuffer(),"draw loaded green")
k=CreateImage(20,20)
SetBuffer ImageBuffer(k)
ClsColor 255,0,255:Cls
Color 255,255,0:Rect 0,0,10,10,1
CheckPixel(1,1,$FFFF00,ImageBuffer(k),"offscreen before unset")
SetBuffer BackBuffer()
CheckPixel(1,1,$FFFF00,ImageBuffer(k),"offscreen before mask")
MaskImage k,255,0,255
CheckPixel(1,1,$FFFF00,ImageBuffer(k),"offscreen after mask")
DrawImage k,200,0
CheckPixel(201,1,$FFFF00,BackBuffer(),"image buffer orientation")
CheckPixel(215,15,$000000,BackBuffer(),"mask transparent")
WritePixel 2,2,$00FFFF,ImageBuffer(k)
CheckPixel(2,2,$00FFFF,ImageBuffer(k),"write pixel")
If SaveImage(k,"graphics-image-buffer.bmp")<>1 Then RuntimeError "Save imagebuffer failed"
f=WriteFile("graphics-smoke-result.txt")
WriteLine f,"PASS grab resize save reload mask offscreen pixel roundtrip"
CloseFile f
End
Function CheckPixel(x,y,wanted,buff,label$)
 got=ReadPixel(x,y,buff) And $FFFFFF
 If got<>wanted Then RuntimeError label$+": got "+got+", expected "+wanted
End Function
