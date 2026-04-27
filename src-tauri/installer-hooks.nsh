; Tauri NSIS installer hooks for GKey Mover
;
; The app hides-to-tray on close, so an upgrade install can race against the
; previous version holding file locks on "GKey Mover.exe". Worse, the
; default Tauri NSIS template also auto-launches the app on the Finish
; page, which can fire while a slow disk is still flushing files and break
; the install.
;
; Defenses in order:
;   1. MUI_FINISHPAGE_RUN_NOTCHECKED — flips the "Run app" checkbox to
;      unchecked so clicking Finish just closes the installer.
;   2. PREINSTALL taskkill — kill any running instance before file copy.
;      Loop with longer sleep because Windows can take a moment to fully
;      release file handles after a process exits.
;   3. POSTINSTALL taskkill — defensive: if anything (Tauri, NSIS, the OS,
;      a pinned shortcut) launched the app while the installer was still
;      working, kill it before the user clicks Finish so they don't end up
;      with a stale instance running off the old binary.

!define MUI_FINISHPAGE_RUN_NOTCHECKED

!macro KILL_GKEY_MOVER
  nsExec::Exec 'taskkill /F /IM "GKey Mover.exe" /T'
  Pop $0
  Sleep 500
  ; Second pass — first taskkill sometimes returns before the OS has
  ; released the file handles, especially on slower disks.
  nsExec::Exec 'taskkill /F /IM "GKey Mover.exe" /T'
  Pop $0
  Sleep 500
!macroend

!macro NSIS_HOOK_PREINSTALL
  DetailPrint "Closing any running GKey Mover instance..."
  !insertmacro KILL_GKEY_MOVER
!macroend

!macro NSIS_HOOK_POSTINSTALL
  ; If anything launched the app while the installer was running, kill it
  ; so the install completes cleanly and the user can launch fresh.
  DetailPrint "Ensuring no GKey Mover instance is running..."
  !insertmacro KILL_GKEY_MOVER
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  DetailPrint "Closing any running GKey Mover instance..."
  !insertmacro KILL_GKEY_MOVER
!macroend
