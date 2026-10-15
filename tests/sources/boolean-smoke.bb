out=WriteFile("boolean-smoke-result.txt")
a=3:b=3
value=(a=b)
If value<>1 Then RuntimeError "dynamic true must equal1; got "+value
value=(a<>b)
If value<>0 Then RuntimeError "dynamic false must equal0"
value=Not (a<>b)
If value<>1 Then RuntimeError "Not must return canonical1"
x#=1.5:y#=1.5
value=(x#=y#)
If value<>1 Then RuntimeError "float comparison true must equal1"
s$="same":t$="same"
value=(s$=t$)
If value<>1 Then RuntimeError "string comparison true must equal1"
value=2:entered=0
While value
 entered=entered+1
 Exit
Wend
If entered<>1 Then RuntimeError "While2 skipped"
value=-2:entered=0
While value
 entered=entered+1
 Exit
Wend
If entered<>1 Then RuntimeError "While-2 skipped"
value=2:entered=0
If value Then entered=1
If entered<>1 Then RuntimeError "If2 skipped"
value=2:entered=0
Repeat
 entered=entered+1
 If entered>2 Then RuntimeError "Until2 repeated"
Until value
If entered<>1 Then RuntimeError "Until2 wrong iteration count"
If ((a=b) And 2)<>0 Then RuntimeError "comparison must use canonical1 in bitwiseAnd"
count=0
For n=3 To 1 Step -1
 count=count+1
Next
If count<>3 Then RuntimeError "descending For"
WriteLine out,"PASS dynamic int/float/string true1/false0; Not; While2/-2; If2; Until2; comparison bitwiseAnd; descending For"
CloseFile out
End
