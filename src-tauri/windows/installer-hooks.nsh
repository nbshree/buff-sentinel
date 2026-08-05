Var LegacyUninstallString
Var MigrateDesktopShortcut

!macro NSIS_HOOK_PREINSTALL
  StrCpy $LegacyUninstallString ""
  StrCpy $MigrateDesktopShortcut 0

  ReadRegStr $LegacyUninstallString HKCU \
    "Software\Microsoft\Windows\CurrentVersion\Uninstall\自动点击流程台" \
    "UninstallString"

  IfFileExists "$DESKTOP\自动点击流程台.lnk" 0 +2
    StrCpy $MigrateDesktopShortcut 1
  IfFileExists "$DESKTOP\${PRODUCTNAME}.lnk" 0 +2
    StrCpy $MigrateDesktopShortcut 1
!macroend

!macro NSIS_HOOK_POSTINSTALL
  ${If} $LegacyUninstallString != ""
    DetailPrint "正在移除旧版自动点击流程台"
    ExecWait '$LegacyUninstallString /S' $R0
  ${EndIf}

  Delete "$DESKTOP\自动点击流程台.lnk"
  Delete "$SMPROGRAMS\自动点击流程台.lnk"
  Delete "$SMPROGRAMS\自动点击流程台\自动点击流程台.lnk"
  RMDir "$SMPROGRAMS\自动点击流程台"

  CreateDirectory "$SMPROGRAMS\${STARTMENUFOLDER}"
  CreateShortcut \
    "$SMPROGRAMS\${STARTMENUFOLDER}\${PRODUCTNAME}.lnk" \
    "$INSTDIR\${MAINBINARYNAME}.exe"

  ${If} $MigrateDesktopShortcut = 1
    CreateShortcut \
      "$DESKTOP\${PRODUCTNAME}.lnk" \
      "$INSTDIR\${MAINBINARYNAME}.exe"
  ${EndIf}
!macroend
