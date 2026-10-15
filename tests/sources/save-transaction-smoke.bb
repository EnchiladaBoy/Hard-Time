phase$=GetEnv$("HARDTIME_SAVE_TEST")
If phase$="initial"
 CreateDir "Data"
 file=WriteFileAtomic("Data/value.dat") : WriteInt file,42 : CloseFile file
 file=WriteFileAtomic("Data/options.dat") : WriteInt file,42 : CloseFile file
 WritePhoto("Data/photo.bmp",0)
EndIf
If phase$="unfinished-file"
 file=WriteFileAtomic("Data/value.dat") : WriteInt file,999
 ; End without CloseFile must not commit a partially written options file.
EndIf
If phase$="unfinished-save"
 BeginSaveTransaction "Data"
 file=WriteFile("data/value.dat") : WriteInt file,999 : CloseFile file
 WritePhoto("data/photo.bmp",1)
EndIf
If phase$="commit"
 RecoverSaveTransaction "Data"
 file=ReadFile("Data/value.dat")
 If ReadInt(file)<>42 Then RuntimeError "Interrupted save replaced committed progress"
 CloseFile file
 BeginSaveTransaction "Data"
 file=WriteFile("data/value.dat") : WriteInt file,84 : CloseFile file
 file=WriteFileAtomic("DATA/options.dat") : WriteInt file,84 : CloseFile file
 WritePhoto("DATA/photo.bmp",1)
 CommitSaveTransaction
EndIf
If phase$="recover-gap"
 RecoverSaveTransaction "Data"
 file=ReadFile("Data/value.dat")
 If ReadInt(file)<>84 Then RuntimeError "Interrupted directory switch was not recovered"
 CloseFile file
EndIf
If phase$="locked-target"
 file=WriteFileAtomic("Data/value.dat") : WriteInt file,777 : CloseFile file
EndIf
file=WriteFile("save-transaction-smoke-result.txt")
WriteLine file,"PASS atomic files, grouped saves and interrupted directory recovery"
CloseFile file
End

Function WritePhoto(path$,green)
 Local photo=CreateImage(16,16)
 SetBuffer ImageBuffer(photo)
 If green Then ClsColor 0,255,0 Else ClsColor 255,0,0
 Cls
 If SaveImage(photo,path$)=0 Then RuntimeError "Save photo fixture failed"
 SetBuffer BackBuffer() : FreeImage photo
End Function
