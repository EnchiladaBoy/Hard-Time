Graphics3D 320,240,32,2
SetBuffer BackBuffer()
camera=CreateCamera()
CameraClsColor camera,0,0,180
font=LoadFont("Arial",16)
SetFont font
image=CreateImage(10,10)
SetBuffer ImageBuffer(image) : ClsColor 0,255,0 : Cls
SetBuffer BackBuffer()
RenderWorld
BeginOverlay
DrawTransform 2,2,10,20
Color 255,0,0 : Rect 10,10,20,20,1
DrawImage image,50,10
Color 255,255,255 : Text 10,50,"Cached text"
Color 255,255,0 : Text 10,75,"Cached text"
EndOverlay
DrawTransform 1,1
baselineImage=CreateImage(320,240) : GrabImage baselineImage,0,0
seed=RndSeed()
For n=1 To 5
 RenderWorld
 If ReplayOverlay()=0 Then RuntimeError "Overlay unexpectedly invalid"
 If RndSeed()<>seed Then RuntimeError "Overlay consumed gameplay randomness"
 LockBuffer BackBuffer() : LockBuffer ImageBuffer(baselineImage)
 For y=0 To 239
  For x=0 To 319
   If ReadPixelFast(x,y,BackBuffer())<>ReadPixelFast(x,y,ImageBuffer(baselineImage)) Then RuntimeError "Overlay replay pixels differ"
  Next
 Next
 UnlockBuffer ImageBuffer(baselineImage) : UnlockBuffer BackBuffer()
Next
If (ReadPixel(35,45,BackBuffer()) And $FFFFFF)<>$FF0000 Then RuntimeError "HUD transform failed"
If (ReadPixel(115,45,BackBuffer()) And $FFFFFF)<>$00FF00 Then RuntimeError "Scaled image failed"
FreeImage image
If OverlayReady() Then RuntimeError "Freed image retained in overlay"
BeginOverlay
Text 0,0,"Font lifetime"
EndOverlay
FreeFont font
If ReplayOverlay() Then RuntimeError "Freed font retained in overlay"
f=WriteFile("overlay-smoke-result.txt")
WriteLine f,"PASS overlay pixels, text cache, transform, RNG and resource invalidation"
CloseFile f
End
