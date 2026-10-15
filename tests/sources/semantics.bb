out=WriteFile("semantics-results.txt")
WriteLine out,"Blitz3D 1.108 numeric conversion ground truth"
WriteLine out,"float | implicit integer assignment | explicit Int(float)"
Restore testvalues
For i=1 To 14
 Read x#
 y=x#
 z=Int(x#)
 WriteLine out,Str$(x#)+" | "+Str$(y)+" | "+Str$(z)
Next
SeedRnd 12345
n0=0:n1=0:n2=0:nother=0
minval=100:maxval=-100
For i=1 To 10000
 draw#=Rnd(1,2)
 r=draw#
 If r=0 Then n0=n0+1
 If r=1 Then n1=n1+1
 If r=2 Then n2=n2+1
 If r<0 Or r>2 Then nother=nother+1
 If r<minval Then minval=r
 If r>maxval Then maxval=r
Next
WriteLine out,"SeedRnd 12345: implicit Rnd(1,2), samples=10000, min="+minval+", max="+maxval+", zero="+n0+", one="+n1+", two="+n2+", other="+nother
SeedRnd 12345
n0=0:n1=0:nother=0
For i=1 To 10000
 draw#=Rnd(0,1)
 r=draw#
 If r=0 Then n0=n0+1
 If r=1 Then n1=n1+1
 If r<0 Or r>1 Then nother=nother+1
Next
WriteLine out,"SeedRnd 12345: implicit Rnd(0,1), samples=10000, zero="+n0+", one="+n1+", other="+nother
CloseFile out
End
.testvalues
Data 0.5,1.5,2.5,3.5,-0.5,-1.5,-2.5,-3.5,1.9,-1.9,0.49,-0.49,0.51,-0.51
