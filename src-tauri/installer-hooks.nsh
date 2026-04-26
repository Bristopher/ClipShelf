; Tauri NSIS installer hooks for GKey Mover
;
; 1. Hide-to-tray means an upgrade install can run while the previous
;    version is still alive and holding file locks on "GKey Mover.exe".
;    PRE{INSTALL,UNINSTALL} taskkill prevents the silent file-overwrite
;    failure that would otherwise leave the install half-applied.
;
; 2. The Tauri default MUI Finish page has a "Run GKey Mover" checkbox
;    that's checked by default — clicking Finish auto-launches the app,
;    which the user perceives as "the installer reopens the app right
;    before it finishes". MUI_FINISHPAGE_RUN_NOTCHECKED flips the
;    default to unchecked so Finish just closes the installer cleanly.
;    Defined here (included at the top of the generated installer.nsi)
;    so it lands before MUI_PAGE_FINISH is processed.

!define MUI_FINISHPAGE_RUN_NOTCHECKED

!macro NSIS_HOOK_PREINSTALL
  DetailPrint "Closing any running GKey Mover instance..."
  nsExec::Exec 'taskkill /F /IM "GKey Mover.exe" /T'
  Pop $0 ; discard exit code — non-zero just means nothing was running
  Sleep 750
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  DetailPrint "Closing any running GKey Mover instance..."
  nsExec::Exec 'taskkill /F /IM "GKey Mover.exe" /T'
  Pop $0
  Sleep 750
!macroend
