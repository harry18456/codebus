; ===========================================================================
; codebus NSIS installer hooks
; ---------------------------------------------------------------------------
; Adds the bundled CLI directory ($INSTDIR\bin) to the *current user* PATH on
; install, and removes exactly that one segment on uninstall. Uninstall also
; offers an OPT-IN full purge (default No) of global user data; see the
; uninstall hook below.
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
;   - by default touches ONLY the PATH environment value; never reads or
;     writes ~/.codebus or any vault .codebus directory
;   - opt-in purge (explicit Yes only) additionally removes the three FIXED
;     global locations ~/.codebus, %LOCALAPPDATA%\com.codebus.app, and the
;     azure keyring entries (via `codebus config purge-keys`); it still NEVER
;     reads, traverses, or deletes any repository's vault .codebus directory,
;     and each step is best-effort (return code ignored, can never block
;     uninstall). nsExec is bundled with NSIS, so the no-plugin rule holds.
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

  ; -------------------------------------------------------------------------
  ; Opt-in full purge (default: No). Everything above this point preserves all
  ; user data; only on an EXPLICIT Yes do we additionally remove global config,
  ; saved credentials, and app data. /SD IDNO makes silent/unattended uninstall
  ; default to No, so automation never deletes user data. Every step is
  ; best-effort with its return code ignored, so a failure or a missing target
  ; can never block or abort the uninstall. We NEVER touch any repository's
  ; vault .codebus/ directory — only the three fixed global locations below.
  ; -------------------------------------------------------------------------
  MessageBox MB_YESNO|MB_DEFBUTTON2|MB_ICONQUESTION \
    "Also remove your codebus settings and saved credentials?$\n$\nYour wikis inside repositories are never touched." \
    /SD IDNO IDYES codebus_purge_yes
  Goto codebus_purge_done

  codebus_purge_yes:
    ; 1. Clear keyring credentials (both providers) BEFORE program files are
    ;    removed, while $INSTDIR\bin\codebus.exe still exists. nsExec is
    ;    bundled with NSIS (no third-party plugin) and blocks until the fast,
    ;    non-interactive purge-keys command exits; its result is discarded.
    nsExec::ExecToLog '"$INSTDIR\bin\codebus.exe" config purge-keys'
    Pop $0   ; discard nsExec return code (best-effort)
    ; 2. Tauri app data (WebView2 cache etc.) for identifier com.codebus.app.
    RMDir /r "$LOCALAPPDATA\com.codebus.app"
    ; 3. Global config + logs.
    RMDir /r "$PROFILE\.codebus"
  codebus_purge_done:

  Pop $1
  Pop $0
!macroend
