sound=LoadSound("tone.wav")
If sound=0 Then RuntimeError "Cannot load audio fixture"
LoopSound sound
SoundGroup sound,2
SoundVolume sound,0.8
channel=PlaySound(sound)
If channel=0 Then RuntimeError "Cannot play audio fixture"
AudioGroupVolume 2,0.25
If Abs(ChannelGain(channel)-0.2)>0.001 Then RuntimeError "Ambience group gain failed"
ChannelVolume channel,0.4
If Abs(ChannelGain(channel)-0.1)>0.001 Then RuntimeError "Per-channel gain replaced category gain"
AudioGroupVolume 0,0
AudioGroupVolume 1,0
If Abs(ChannelGain(channel)-0.1)>0.001 Then RuntimeError "Unrelated group changed ambience"
AudioGroupVolume 2,0
If ChannelGain(channel)<>0 Or ChannelPlaying(channel)=0 Then RuntimeError "Muted channel lost playback state"
SoundVolume sound,0
silent=PlaySound(sound)
AudioGroupVolume 2,1
If ChannelGain(silent)<>0 Or ChannelPlaying(silent)=0 Then RuntimeError "Volume slider unmuted silent alarm"
f=WriteFile("audio-groups-smoke-result.txt")
WriteLine f,"PASS independent category gains, live sliders and silent channel state"
CloseFile f
End
