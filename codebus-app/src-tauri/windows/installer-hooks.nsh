; ===========================================================================
; codebus NSIS installer hooks
; ---------------------------------------------------------------------------
; Adds the bundled CLI directory ($INSTDIR\bin) to the *current user* PATH on
; install, and removes exactly that one segment on uninstall.
;
; Referenced by tauri.conf.json -> bundle.windows.nsis.installerHooks and
; !included at global scope into Tauri's generated installer.nsi (which already
; includes StrFunc.nsh + LogicLib).
;
; Safety contract (openspec change windows-installer-foundation, decision 3/4):
;   - per-user only: HKCU\Environment, never HKLM, never requires admin
;   - native registry only: no EnVar / third-party plugin (no !addplugindir)
;   - idempotent add: never append a duplicate segment
;   - surgical remove: strip only our segment, never rewrite PATH wholesale
;     and never truncate the rest of the user's PATH
;   - REG_EXPAND_SZ via WriteRegExpandStr (preserves %VAR% expansion)
;   - broadcast WM_SETTINGCHANGE so already-open shells refresh
;   - touches ONLY the PATH environment value; never reads or writes
;     ~/.codebus or any vault .codebus directory
; ===========================================================================

; Instantiate the StrFunc helpers we use. Tauri's template already includes
; StrFunc.nsh and uses ${StrCase}/${StrLoc}; StrStr/StrRep are unused upstream,
; so declaring them here is safe.
${StrStr}     ; installer: substring search for idempotency
${UnStrRep}   ; uninstaller: surgical segment removal

!define CODEBUS_HWND_BROADCAST 0xFFFF
!define CODEBUS_WM_SETTINGCHANGE 0x1A

; Notify running processes that the per-user environment changed, so newly
; spawned shells (and Explorer-launched processes) see the updated PATH without
; a logout. SMTO_ABORTIFHUNG (2) + 5s timeout keeps the installer responsive.
!macro CodebusBroadcastEnv
  System::Call 'user32::SendMessageTimeoutW(p ${CODEBUS_HWND_BROADCAST}, i ${CODEBUS_WM_SETTINGCHANGE}, p 0, t "Environment", i 2, i 5000, *p .r0)'
!macroend

; ---------------------------------------------------------------------------
; Install: append "$INSTDIR\bin" to HKCU PATH, idempotently.
; ---------------------------------------------------------------------------
!macro NSIS_HOOK_POSTINSTALL
  Push $0   ; current PATH value
  Push $1   ; target dir ($INSTDIR\bin)
  Push $2   ; StrStr result

  StrCpy $1 "$INSTDIR\bin"
  ReadRegStr $0 HKCU "Environment" "Path"

  ${If} $0 == ""
    ; PATH unset/empty -> write just our dir, no leading separator.
    WriteRegExpandStr HKCU "Environment" "Path" "$1"
    !insertmacro CodebusBroadcastEnv
  ${Else}
    ; Only add when absent. A substring hit is safe-failing: at worst we skip
    ; adding (CLI not on PATH, caught in real-machine verification) — it can
    ; never corrupt the existing PATH.
    ${StrStr} $2 "$0" "$1"
    ${If} $2 == ""
      WriteRegExpandStr HKCU "Environment" "Path" "$0;$1"
      !insertmacro CodebusBroadcastEnv
    ${EndIf}
  ${EndIf}

  Pop $2
  Pop $1
  Pop $0
!macroend

; ---------------------------------------------------------------------------
; Uninstall: remove ONLY our segment from HKCU PATH. Separator-bounded
; replacements (";dir" / "dir;") cannot clip an unrelated entry; the exact-match
; case is handled explicitly so a bare substring is never blindly stripped.
; ---------------------------------------------------------------------------
!macro NSIS_HOOK_PREUNINSTALL
  Push $0   ; current PATH value
  Push $1   ; target dir ($INSTDIR\bin)

  StrCpy $1 "$INSTDIR\bin"
  ReadRegStr $0 HKCU "Environment" "Path"

  ${If} $0 != ""
    ${If} $0 == "$1"
      ; PATH was exactly our dir -> clear it.
      StrCpy $0 ""
    ${Else}
      ; Middle/end occurrence ";dir", then start occurrence "dir;".
      ${UnStrRep} $0 "$0" ";$1" ""
      ${UnStrRep} $0 "$0" "$1;" ""
    ${EndIf}
    WriteRegExpandStr HKCU "Environment" "Path" "$0"
    !insertmacro CodebusBroadcastEnv
  ${EndIf}

  Pop $1
  Pop $0
!macroend
