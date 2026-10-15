Include "NativeBars.bb"
For mode=1 To 5
 If mode=1 Then width=800 : height=600
 If mode=2 Then width=1280 : height=720
 If mode=3 Then width=1280 : height=1024
 If mode=4 Then width=2560 : height=1080
 If mode=5 Then width=3840 : height=2160
 Graphics3D width,height,32,2
 SetBuffer BackBuffer()
 If GraphicsWidth()<>width Or GraphicsHeight()<>height Then RuntimeError "Wrong drawable size"
 For stage=1 To 5
  If stage=1 Then ticks=0
  If stage=2 Then ticks=12
  If stage=3 Then ticks=25
  If stage=4 Then ticks=9988
  If stage=5 Then ticks=10000
  DrawTransform 1,1,0,0 : ClsColor 30,90,180 : Cls
  BeginOverlay
  NativeDialogueBars(ticks)
  ; The helper must restore logical UI coordinates for subtitles.
  Color 255,255,255 : Rect 390,550,20,10,1
  EndOverlay
  For replay=0 To 1
   If replay Then DrawTransform 1,1,0,0 : Cls : ReplayOverlay()
   For side=0 To 1
    x=side*(width-1)
    If ticks=0 Or ticks=10000
     If (ReadPixel(x,0,BackBuffer()) And $FFFFFF)<>$1E5AB4 Then RuntimeError "Closed top bar is visible"
     If (ReadPixel(x,height-1,BackBuffer()) And $FFFFFF)<>$1E5AB4 Then RuntimeError "Closed bottom bar is visible"
    Else
     If (ReadPixel(x,0,BackBuffer()) And $FFFFFF)<>0 Then RuntimeError "Top bar does not span the display"
     If (ReadPixel(x,height-1,BackBuffer()) And $FFFFFF)<>0 Then RuntimeError "Bottom bar does not span the display"
    EndIf
    If (ReadPixel(x,height/2,BackBuffer()) And $FFFFFF)<>$1E5AB4 Then RuntimeError "Bars cover the scene"
   Next
   scale#=width/800.0 : If height/600.0<scale# Then scale#=height/600.0
   textY=(height-600*scale#)/2+555*scale#
   If (ReadPixel(width/2,textY,BackBuffer()) And $FFFFFF)<>$FFFFFF Then RuntimeError "Subtitle transform was not restored"
   If stage=3
    ; Check coverage at the original text baseline, beyond the 4:3 safe area.
    If (ReadPixel(0,textY,BackBuffer()) And $FFFFFF)<>0 Then RuntimeError "Subtitle bar stops at UI boundary"
    If (ReadPixel(width-1,textY,BackBuffer()) And $FFFFFF)<>0 Then RuntimeError "Subtitle bar has a right-side gap"
   EndIf
  Next
 Next
 ClearOverlay
Next
f=WriteFile("dialogue-bars-smoke-result.txt")
WriteLine f,"PASS full-width animated bars, subtitle transform and overlay replay at five aspect/resolution combinations"
CloseFile f
End
