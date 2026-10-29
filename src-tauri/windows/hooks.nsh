; Hooks into the installer Tauri generates; see bundle.windows.nsis.installerHooks.

!macro NSIS_HOOK_POSTINSTALL
  ; tTerm adds the tterm command, unless the user turned it off in its
  ; settings, and the folder menu if they turned it on. Installing over an
  ; older version uninstalls that first, which takes both out, and this puts
  ; them back; an update also brings the command to those who never turned it
  ; on. Settings can add them later, so a failure does not stop the install.
  Push $0
  ClearErrors
  ExecWait '"$INSTDIR\${MAINBINARYNAME}.exe" --add-system-integration' $0
  ${If} ${Errors}
  ${OrIf} $0 <> 0
    DetailPrint "Could not add the tterm command or the folder menu. Turn them on in tTerm's settings under System integration."
  ${EndIf}
  Pop $0
!macroend

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
