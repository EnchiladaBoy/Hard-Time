Global output=WriteFile("rounding-smoke-result.txt")
Check("0.5",0.5)
Check("1.5",1.5)
Check("2.5",2.5)
Check("-0.5",-0.5)
Check("-1.5",-1.5)
Check("-2.5",-2.5)
Check("1.9",1.9)
Check("-1.9",-1.9)
a%=0.5 : b%=1.5 : c%=2.5 : d%=-0.5 : e%=-1.5 : f%=-2.5
WriteLine output,"Constants="+a+","+b+","+c+","+d+","+e+","+f
SeedRnd 12345
zeros=0 : ones=0 : lowerCount=0 : upperCount=0
For i=1 To 10000
 value=Rnd(0,1)
 If value=0 Then zeros=zeros+1 Else ones=ones+1
 value=Rnd(1,2)
 If value=1 Then lowerCount=lowerCount+1 Else upperCount=upperCount+1
Next
WriteLine output,"Random="+zeros+","+ones+","+lowerCount+","+upperCount
If zeros=0 Or ones=0 Or lowerCount=0 Or upperCount=0 Then RuntimeError "Float to integer random choices failed"
WriteLine output,"PASS"
CloseFile output
End
Function Check(label$,value#)
 implicit%=value#
 explicit%=Int(value#)
 WriteLine output,label$+"="+implicit+","+explicit
End Function
