; Hooks into the installer Tauri generates; see bundle.windows.nsis.installerHooks.

!macro NSIS_HOOK_PREUNINSTALL
  ; tTerm takes out the tterm command and the folder menu it added, while its
  ; executable is still there to do it. An update runs the uninstaller too, and
  ; keeps them.
  ${If} $UpdateMode <> 1
    ExecWait '"$INSTDIR\${MAINBINARYNAME}.exe" --remove-system-integration'
  ${EndIf}
!macroend
