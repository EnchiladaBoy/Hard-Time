For iteration=1 To 5
 Graphics3D 320+iteration*8,240,32,2
 SetBuffer BackBuffer()
 camera=CreateCamera()
 For cycle=1 To 40
  mesh=CreateCube()
  EntityFX mesh,1 : EntityColor mesh,255,0,0 : PositionEntity mesh,0,0,5
  RenderWorld
  If (ReadPixel(GraphicsWidth()/2,120,BackBuffer()) And $FFFFFF)<>$FF0000 Then RuntimeError "Mesh lifecycle corrupted draw buffers"
  FreeEntity mesh
 Next
 FreeEntity camera
Next
retainedImage=CreateImage(16,16)
SetBuffer ImageBuffer(retainedImage) : ClsColor 0,255,0 : Cls
SetBuffer BackBuffer()
If ResizeGraphics(640,480,0)=0 Then RuntimeError "Live window resize failed"
DrawImage retainedImage,10,10
If (ReadPixel(15,15,BackBuffer()) And $FFFFFF)<>$00FF00 Then RuntimeError "Resize invalidated an image"
If ResizeGraphics(DesktopWidth(),DesktopHeight(),1)=0 Then RuntimeError "Live borderless transition failed"
If GraphicsWidth()<>DesktopWidth() Or GraphicsHeight()<>DesktopHeight() Then RuntimeError "Live desktop dimensions differ"
If ResizeGraphics(640,480,0)=0 Then RuntimeError "Live window restore failed"
DrawImage retainedImage,10,10
If (ReadPixel(15,15,BackBuffer()) And $FFFFFF)<>$00FF00 Then RuntimeError "Mode switch invalidated an image"
Graphics3D DesktopWidth(),DesktopHeight(),32,4
If GraphicsWidth()<>DesktopWidth() Or GraphicsHeight()<>DesktopHeight() Then RuntimeError "Borderless dimensions differ from desktop"
Graphics3D 640,480,32,2
If GraphicsWidth()<>640 Or GraphicsHeight()<>480 Then RuntimeError "Windowed mode did not restore"
f=WriteFile("display-lifecycle-smoke-result.txt")
WriteLine f,"PASS repeated mesh/context lifetimes, borderless desktop and windowed restore"
CloseFile f
End
