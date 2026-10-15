Graphics3D 160,120,32,2
SetBuffer BackBuffer()
camera=CreateCamera()
CameraClsColor camera,0,0,255
CheckGray("black",0,2,0,255)
CheckGray("white",255,2,255,0)
CheckGray("gray",128,2,64,127)
CheckGray("masked",0,4,0,255)
CheckGray("opaque",0,1,0,0)
CheckSprite("alpha-rgba.png",2,64,191)
f=WriteFile("texture-alpha-smoke-result.txt")
WriteLine f,"PASS grayscale alpha, black mask, opaque RGB, explicit PNG alpha"
CloseFile f
End

Function CheckGray(name$,shade,flags,red,blue)
 image=CreateImage(8,8)
 SetBuffer ImageBuffer(image)
 ClsColor shade,shade,shade:Cls
 If SaveImage(image,name$+".bmp")<>1 Then RuntimeError "Cannot write alpha fixture"
 SetBuffer BackBuffer()
 FreeImage image
 CheckSprite(name$+".bmp",flags,red,blue)
End Function

Function CheckSprite(file$,flags,red,blue)
 sprite=LoadSprite(file$,flags)
 If sprite=0 Then RuntimeError "Cannot load alpha fixture"
 PositionEntity sprite,0,0,3
 EntityColor sprite,255,0,0
 EntityBlend sprite,1
 RenderWorld
 pixel=ReadPixel(80,60,BackBuffer())
 r=(pixel Shr 16) And 255
 g=(pixel Shr 8) And 255
 b=pixel And 255
 If Abs(r-red)>3 Or g<>0 Or Abs(b-blue)>3 Then RuntimeError file$+": incorrect alpha blending "+r+","+g+","+b
 FreeEntity sprite
End Function
