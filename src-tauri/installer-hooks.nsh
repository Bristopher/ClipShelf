; Tauri NSIS installer hooks for GKey Mover
;
; The app hides-to-tray on window close, so an upgrade install can run while
; the previous version is still alive and holding file locks on
; "GKey Mover.exe". Without this hook NSIS silently fails to overwrite the
; binary and the install ends in a broken state.
;
; PREINSTALL: terminate any running instance before copying files. taskkill's
; /F is needed to bypass the tray icon's hidden window message loop.
; PREUNINSTALL: same reason, for clean uninstall.

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
