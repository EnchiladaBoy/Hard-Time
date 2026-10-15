Global optRes=2,keyAttack=30,keyDefend=44,keyThrow=31,keyPickUp=45,sAlarm=0
Dim gamName$(3)
Include "NativePreferences.bb"
CreateDir "Data"
NativeLoadExtras()
If nativeMaster<>100 Or nativeMute Or nativeLimbPunch Or nativeGodMode Or nativeSuperPunch Then RuntimeError "Defaults changed"
Dim sound(2),channel(2)
For n=0 To 2
 sound(n)=LoadSound("tone.wav") : SoundGroup sound(n),n : LoopSound sound(n)
 SoundVolume sound(n),0.8 : channel(n)=PlaySound(sound(n))
Next
sAlarm=LoadSound("tone.wav") : LoopSound sAlarm : SoundVolume sAlarm,0
alarm=PlaySound(sAlarm)
nativeMaster=50 : nativeEffects=40 : nativeMusic=60 : nativeAmbience=20
NativeApplyAudio()
If Abs(ChannelGain(channel(0))-0.16)>0.001 Then RuntimeError "Master/effects multiplication failed"
If Abs(ChannelGain(channel(1))-0.24)>0.001 Then RuntimeError "Master/music multiplication failed"
If Abs(ChannelGain(channel(2))-0.08)>0.001 Then RuntimeError "Master/ambience multiplication failed"
nativeMute=1 : NativeApplyAudio()
For n=0 To 2
 If ChannelGain(channel(n))<>0 Or ChannelPlaying(channel(n))=0 Then RuntimeError "Mute must preserve playing channels"
Next
nativeMute=0 : NativeApplyAudio()
If Abs(ChannelGain(channel(0))-0.16)>0.001 Then RuntimeError "Unmute lost previous gain"
nativeMaster=0 : NativeApplyAudio()
If ChannelGain(channel(1))<>0 Then RuntimeError "Zero master is audible"
nativeMaster=100 : nativeEffects=100 : NativeApplyAudio()
If ChannelGain(alarm)<>0 Or ChannelPlaying(alarm)=0 Then RuntimeError "Settings unmuted the alarm"
nativeMaster=45 : nativeMute=1 : nativeLimbPunch=1 : nativeGodMode=1 : nativeSuperPunch=1
NativeSaveSettings()
If FileSize("Data/NativeOptions.dat")<>84 Then RuntimeError "Legacy modern settings format changed"
nativeMaster=100 : nativeMute=0 : nativeLimbPunch=0 : nativeGodMode=0 : nativeSuperPunch=0
NativeLoadSettings()
If nativeMaster<>45 Or nativeMute<>1 Or nativeLimbPunch<>1 Or nativeGodMode<>1 Or nativeSuperPunch<>1 Then RuntimeError "Extra preferences round trip failed"
NativeSaveExtras()
f=WriteFile("Data/NativeExtras.dat") : WriteInt f,0 : CloseFile f
NativeLoadExtras()
If nativeMaster<>45 Or nativeGodMode<>1 Then RuntimeError "Extra settings backup recovery failed"
If FileSize("Data/NativeExtras.corrupt.dat")<>4 Then RuntimeError "Corrupt preferences were not preserved"
; A checksum-valid but out-of-range toggle must still be refused.
f=WriteFile("Data/NativeExtras.dat")
WriteInt f,1213484357 : WriteInt f,1
WriteInt f,45 : WriteInt f,1 : WriteInt f,2 : WriteInt f,1 : WriteInt f,1
WriteInt f,1213484357 Xor 45 Xor 1 Xor 2 Xor 1 Xor 1 : CloseFile f
If NativeReadExtras("Data/NativeExtras.dat") Then RuntimeError "Invalid toggle accepted"
DeleteFile "Data/NativeExtras.dat.bak"
NativeLoadExtras()
If nativeMaster<>100 Or nativeLimbPunch Or nativeGodMode Or nativeSuperPunch Then RuntimeError "Corrupt cheats must default off"
f=WriteFile("audio-settings-smoke-result.txt")
WriteLine f,"PASS live master/category gain, mute/unmute, silent alarm, defaults, persistence and corrupt-settings recovery"
CloseFile f
End
