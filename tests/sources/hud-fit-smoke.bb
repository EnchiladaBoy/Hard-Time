Graphics3D 320,240,32,2
Global out=WriteFile("hud-fit-smoke-result.txt")
For resolution=1 To 5
 If resolution=1 Then width=640
 If resolution=2 Then width=800
 If resolution=3 Then width=1024
 If resolution=4 Then width=1280
 If resolution=5 Then width=1600
 labelFont=LoadFont("Liberation Sans",12)
 numberFont=LoadFont("Liberation Sans:style=Bold",15)
 SetFont numberFont
 numberWidth=StringWidth("100%")
 SetFont labelFont
 CheckFit(width,"Strength",46,StringWidth("Strength:"),numberWidth)
 CheckFit(width,"Agility",40,StringWidth("Agility:"),numberWidth)
 CheckFit(width,"Intelligence",54,StringWidth("Intelligence:"),numberWidth)
 CheckFit(width,"Reputation",54,StringWidth("Reputation:"),numberWidth)
 CheckFit(width,"top pair",43,StringWidth("Agility:"),numberWidth)
 CheckFit(width,"bottom pair",55,StringWidth("Reputation:"),numberWidth)
 FreeFont labelFont
 FreeFont numberFont
Next
WriteLine out,"PASS"
CloseFile out
End
Function CheckFit(width,label$,distance#,labelWidth,numberWidth)
 gap#=distance#-(labelWidth+numberWidth)/2.0-3
 WriteLine out,width+" "+label$+" outline-gap="+gap#
 If gap#<0 Then RuntimeError "HUD text overlaps"
End Function
