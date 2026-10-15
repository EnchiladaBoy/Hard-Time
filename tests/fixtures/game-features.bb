; Runs against all actual prepared game functions and original character models.
Function NativeFeatureChecks()
 Local attacker=gamPlayer(slot),target=0,corpse=0,n,hit,root,liveDebris=0,oldHealth
 For n=1 To no_plays
  If n<>attacker And pHealth(n)>0 Then target=n
  If n<>attacker And pHealth(n)=0 Then corpse=n
 Next
 If target=0 Or corpse=0 Then RuntimeError "Missing live/corpse combat fixtures"
 gotim=100 : gamPromo=0 : nativeLimbPunch=0 : nativeGodMode=0 : nativeSuperPunch=0
 ; Exercise the actual punch contact/damage branch, with cheats disabled first.
 pHealth(target)=1
 NativeTestPunch(attacker,target)
 If pHealth(target)>0 Then RuntimeError "Baseline punch did not land or became nonlethal"
 If pScar(target,4)=5 Then RuntimeError "Cheats changed combat while disabled"
 ; Misses, blocks, weapons and NPC attacks must not trigger the cheat.
 pHealth(target)=100 : nativeLimbPunch=1
 NativeMeleeHit(attacker,target,1)
 If pScar(target,4)=5 Then RuntimeError "Blocked hit removed a limb"
 pWeapon(attacker)=1 : NativeMeleeHit(attacker,target,0) : pWeapon(attacker)=0
 If pScar(target,4)=5 Then RuntimeError "Weapon hit triggered unarmed cheat"
 NativeMeleeHit(target,attacker,0)
 If pScar(attacker,4)=5 Then RuntimeError "NPC received the player's cheat"
 ; Four confirmed hits sever all four major limbs; lethal damage is intercepted.
 nativeSuperPunch=1
 For hit=1 To 4
  pHealth(target)=1
  NativeTestPunch(attacker,target)
  root=4
  If hit=2 Then root=17
  If hit=3 Then root=31
  If hit=4 Then root=34
  If pScar(target,root)<>5 Then RuntimeError "Confirmed punch did not detach expected limb"
  If pHealth(target)<25 Or charHealth(pChar(target))<25 Then RuntimeError "Nonlethal hit killed target"
  If pStagger#(target)<5.9 Then RuntimeError "Super knockback was overwritten"
 Next
 If pScar(target,1)=5 Or pScar(target,3)=5 Or pScar(target,30)=5 Then RuntimeError "Cheat detached a vital body part"
 For n=4 To 29
  If pScar(target,n)<>5 Then RuntimeError "Detached arm descendants would reappear after reload"
 Next
 For n=31 To 36
  If pScar(target,n)<>5 Then RuntimeError "Detached leg descendants would reappear after reload"
 Next
 For n=1 To 24
  If nativeDebris(n)>0 Then liveDebris=liveDebris+1
 Next
 If liveDebris<>4 Then RuntimeError "Missing detached limb effects"
 ; Verify both dying-animation guards, repeated damage/bleeding and existing corpses.
 For n=1 To 240
  pHealth(target)=-10 : pAnim(target)=76 : pAnimTim(target)=149
  NativeUpdateCheats()
  If pHealth(target)<25 Or pAnim(target)=76 Then RuntimeError "Protected NPC entered death state"
  If pHealth(corpse)<>0 Then RuntimeError "Cheat revived an existing corpse"
 Next
 For n=1 To 24
  If nativeDebris(n)>0 Then RuntimeError "Detached limb effects were not retired"
 Next
 nativeGodMode=1 : pHealth(attacker)=-10 : pInjured(attacker)=5000 : pAnim(attacker)=77 : pAnimTim(attacker)=149
 NativeUpdateCheats()
 If pHealth(attacker)<>100 Or pInjured(attacker)<>0 Or pAnim(attacker)=77 Then RuntimeError "Player invincibility failed"
 For n=1 To 50 : ScarLimb(attacker,4,0) : RiskInjury(attacker,0) : Next
 LoseLimb(attacker,4,0,1)
 If pScar(attacker,4)<>0 Or pInjured(attacker)<>0 Then RuntimeError "Invincible player received an injury"
 nativeGodMode=0 : nativeLimbPunch=0 : pHealth(target)=-5
 NativeUpdateCheats()
 If pHealth(target)<>-5 Then RuntimeError "Disabling cheats retained health protection"
 If pScar(target,4)<>5 Then RuntimeError "Disabling cheats unexpectedly restored saved limbs"
 ; Real character serialization retains limb loss and positive health.
 nativeLimbPunch=1 : NativeUpdateCheats()
 For n=1 To 40 : charScar(pChar(target),n)=pScar(target,n) : Next
 SaveChars()
 charScar(pChar(target),4)=0 : charHealth(pChar(target))=0
 LoadChars()
 If charScar(pChar(target),4)<>5 Or charHealth(pChar(target))<25 Then RuntimeError "Character save lost nonlethal dismemberment"
 NativeClearDebris()
 Local output=WriteFile("game-features-result.txt")
 WriteLine output,"PASS real punch hooks, disabled/block/weapon/NPC guards, four limbs, survival, corpse exclusion, invincibility, debris cleanup and save roundtrip"
 CloseFile output
 End
End Function

Function NativeTestPunch(attacker,target)
 pFoc(attacker)=target : pFoc(target)=attacker
 pX#(attacker)=0 : pY#(attacker)=20 : pZ#(attacker)=0 : pA#(attacker)=0
 pX#(target)=0 : pY#(target)=20 : pZ#(target)=8 : pA#(target)=180
 PositionEntity p(attacker),0,20,0 : PositionEntity pPivot(attacker),0,38,0
 PositionEntity p(target),0,20,8 : PositionEntity pPivot(target),0,38,8
 RotateEntity p(attacker),0,0,0 : RotateEntity pPivot(attacker),0,0,0
 RotateEntity p(target),0,180,0
 pAnim(attacker)=30 : pAnimTim(attacker)=6 : pSting(attacker)=1
 pAnim(target)=0 : pAnimTim(target)=0
 SeedRnd 100
 Local log=OpenFile("punch-debug.txt")
 If log=0 Then log=WriteFile("punch-debug.txt")
 SeekFile log,FileSize("punch-debug.txt")
 WriteLine log,"before attacker="+attacker+" target="+target+" viable="+AttackViable(target)+" range="+InRange(attacker,target,3)+" scar="+pScar(attacker,18)+" weapon="+pWeapon(attacker)
 Animations(attacker)
 WriteLine log,"after sting="+pSting(attacker)+" anim="+pAnim(attacker)+" target health="+pHealth(target)+" limb="+pScar(target,4)
 CloseFile log
 If pSting(attacker)<>0 Then RuntimeError "Test punch missed its target"
End Function
