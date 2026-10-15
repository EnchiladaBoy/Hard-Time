Graphics3D 160,120,32,2
SetBuffer BackBuffer()
camera=CreateCamera()
CameraClsColor camera,0,0,255
tex=CreateTexture(8,8,1)
SetBuffer TextureBuffer(tex)
ClsColor 255,0,0:Cls
SetBuffer BackBuffer()
For layer=0 To 7
 cube=CreateCube()
 PositionEntity cube,0,0,4
 EntityFX cube,1
 EntityTexture cube,tex,0,layer
 RenderWorld
 got=ReadPixel(80,60,BackBuffer()) And $FFFFFF
 If got<>$FF0000 Then RuntimeError "texture layer "+layer+": got "+got+", expected red"
 FreeEntity cube
Next
; Disabled stages must not disrupt the next active stage's sampler.
disabled=CreateTexture(8,8,1)
cube=CreateCube()
PositionEntity cube,0,0,4
EntityFX cube,1
TextureBlend disabled,0
EntityTexture cube,disabled,0,0
EntityTexture cube,tex,0,6
RenderWorld
got=ReadPixel(80,60,BackBuffer()) And $FFFFFF
If got<>$FF0000 Then RuntimeError "disabled stage: got "+got+", expected red"
FreeEntity cube
cube=CreateCube()
PositionEntity cube,0,0,4
EntityFX cube,1
pending=CreateTexture(8,8,1)
TextureBlend pending,3
EntityTexture cube,tex,0,1
EntityTexture cube,pending,0,6
RenderWorld
got=ReadPixel(80,60,BackBuffer()) And $FFFFFF
If got<>$FF0000 Then RuntimeError "lazy second stage: got "+got+", expected red"
f=WriteFile("material-smoke-result.txt")
WriteLine f,"PASS texture layers 0..7, disabled stage before active layer6, lazy second-stage upload"
CloseFile f
End
