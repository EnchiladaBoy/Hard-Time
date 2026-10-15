; Test actual rendered pixels, not just transform arithmetic.
Graphics3D 160,120,32,2
SetBuffer BackBuffer()
cam=CreateCamera()
CameraClsColor cam,0,0,255
parent=CreatePivot()
cube=CreateCube(parent)
ScaleEntity cube,0.2,0.2,0.2
EntityColor cube,255,0,0
EntityFX cube,1
PositionEntity parent,-2,0,5
CaptureWorld
PositionEntity parent,2,0,5
For sample=0 To 2
 RenderWorld sample/2.0
 pixel=ReadPixel(80,60,BackBuffer()) And $FFFFFF
 If sample=1 And pixel<>$FF0000 Then RuntimeError "Hierarchy interpolation midpoint missing"
 If sample<>1 And pixel<>$0000FF Then RuntimeError "Endpoint incorrectly at midpoint"
 If EntityX(parent)<>2 Then RuntimeError "Rendering changed simulation transform"
Next
f=WriteFile("interpolation-smoke-result.txt")
WriteLine f,"PASS parent interpolation pixels, unchanged simulation transforms"
CloseFile f
End
