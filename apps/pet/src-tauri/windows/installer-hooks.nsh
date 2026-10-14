; Keep definitions during an in-place upgrade. A full uninstall removes only our entries.
!macro NSIS_HOOK_PREUNINSTALL
  ClearErrors
  ${GetOptions} $CMDLINE "/UPDATE" $R0
  ${If} ${Errors}
    IfFileExists "$INSTDIR\code-musume-hook.exe" 0 cleanup_done
    nsExec::ExecToLog '"$INSTDIR\code-musume-hook.exe" cleanup-installs --runtime release'
    Pop $R0
    ${If} $R0 != 0
      MessageBox MB_OK|MB_ICONEXCLAMATION "Hook cleanup failed. Please remove the connections in Code Musume settings before uninstalling."
      Abort
    ${EndIf}
    cleanup_done:
    DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "CodeMusume"
  ${EndIf}
!macroend
