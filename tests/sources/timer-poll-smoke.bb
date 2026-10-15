Graphics3D 160,120,32,2
SetBuffer BackBuffer()
cam=CreateCamera()
cube=CreateCube()
PositionEntity cube,0,0,5
; Warm up the renderer before measuring the scheduler.
RenderWorld
Flip 0
timer=CreateTimer(30)
start=MilliSecs()
ticks=0 : renders=0 : emptyPolls=0
While MilliSecs()-start<1000
 count=PollTimer(timer)
 tween#=TimerTween#(timer)
 If tween#<0 Or tween#>=1 Then RuntimeError "Invalid interpolation fraction"
 If count=0 Then emptyPolls=emptyPolls+1
 ticks=ticks+count
 For i=1 To count
  CaptureWorld
  TurnEntity cube,0,1,0
  UpdateWorld
 Next
 RenderWorld tween#
 Flip 0
 renders=renders+1
Wend
ticks=ticks+PollTimer(timer)
elapsed=MilliSecs()-start
; Poll and Wait share one tick stream: neither loses ticks after a stall.
Delay 120
catchup=PollTimer(timer)
waited=WaitTimer(timer)
FreeTimer timer
f=WriteFile("timer-poll-smoke-result.txt")
WriteLine f,"Ticks="+ticks
WriteLine f,"Renders="+renders
WriteLine f,"EmptyPolls="+emptyPolls
WriteLine f,"Elapsed="+elapsed
WriteLine f,"Catchup="+catchup
WriteLine f,"Waited="+waited
CloseFile f
End
