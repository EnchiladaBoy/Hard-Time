; NativePreferences.bb is copied from the prepared game, never reimplemented.
Global optRes=2,optPopulation=60,optFog=1,optShadows=2,optFX=1,optGore=3
Global keyAttack=30,keyDefend=44,keyThrow=31,keyPickUp=45,sAlarm=0
Dim gamName$(3)
Include "NativePreferences.bb"
CreateDir "Data"
NativeKeyPreset(1)
nativeMusic=35 : nativeEffects=65 : nativeAmbience=20
nativeHud=115 : nativeSensitivity=175 : nativeDistance=120
optRes=9 : nativeBorderless=1
NativeSaveSettings()
If NativeReadSettings("Data/NativeOptions.dat")=0 Then RuntimeError "Valid settings rejected"
NativeKeyPreset(0) : nativeMusic=100 : optRes=2
NativeLoadSettings()
If nativeKey(1)<>17 Or nativeKey(5)<>36 Or nativeMusic<>35 Or optRes<>9 Then RuntimeError "Settings round trip changed values"
NativeSaveSettings()
file=WriteFile("Data/NativeOptions.dat") : WriteInt file,1 : CloseFile file
NativeLoadSettings()
If nativeSettingsRecovered=0 Or nativeKey(1)<>17 Or nativeMusic<>35 Then RuntimeError "Truncated settings did not recover backup"
file=OpenFile("Data/NativeOptions.dat") : SeekFile file,8 : WriteInt file,999 : CloseFile file
If NativeReadSettings("Data/NativeOptions.dat") Then RuntimeError "Corrupt setting accepted"
file=WriteFile("Data/Options.dat")
WriteInt file,2 : WriteInt file,60 : WriteInt file,1 : WriteInt file,2 : WriteInt file,1 : WriteInt file,3
WriteInt file,30 : WriteInt file,44 : WriteInt file,31 : WriteInt file,45
WriteInt file,1 : WriteInt file,3 : WriteInt file,2 : WriteInt file,4
WriteString file,"Slot one" : WriteString file,"" : WriteString file,"Slot three"
CloseFile file
If NativeValidOptions("Data/Options.dat")=0 Then RuntimeError "Valid legacy options rejected"
CopyFile "Data/Options.dat","Data/Options.dat.bak"
file=OpenFile("Data/Options.dat") : SeekFile file,56 : WriteInt file,$7FFFFFFF : CloseFile file
If NativeValidOptions("Data/Options.dat") Then RuntimeError "Unbounded string length accepted"
If NativeRecoverOptions()=0 Then RuntimeError "Legacy options backup did not recover"
If NativeValidOptions("Data/Options.dat")=0 Then RuntimeError "Recovered legacy options invalid"
file=WriteFile("preferences-smoke-result.txt")
WriteLine file,"PASS preferences round trip, bounds, truncated files and backup recovery"
CloseFile file
End
