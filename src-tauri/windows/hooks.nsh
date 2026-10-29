; Hooks into the installer Tauri generates; see bundle.windows.nsis.installerHooks.

!macro NSIS_HOOK_PREUNINSTALL
  ; tTerm takes out the tterm command and the folder menu it added, while its
  ; executable is still there to do it. An update runs the uninstaller too, and
  ; keeps them.
  ${If} $UpdateMode <> 1
    Push $0
    ClearErrors
    ExecWait '"$INSTDIR\${MAINBINARYNAME}.exe" --remove-system-integration' $0
    ; Uninstalling goes on regardless, so the user learns what may be left to
    ; remove by hand.
    ${If} ${Errors}
    ${OrIf} $0 <> 0
      MessageBox MB_ICONEXCLAMATION|MB_OK "tTerm could not remove everything it added to Windows. If they are still there, remove %LOCALAPPDATA%\com.stone.tTerm\bin from your PATH in Environment Variables, and delete the tTerm keys under HKEY_CURRENT_USER\Software\Classes\Directory\shell, Directory\Background\shell and Drive\shell in Registry Editor." /SD IDOK
    ${EndIf}
    Pop $0
  ${EndIf}
!macroend
