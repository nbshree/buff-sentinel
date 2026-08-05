Var LegacyUninstallString
Var LegacyInstallDir
Var CurrentUninstallString
Var CurrentInstallDir
Var MigrateDesktopShortcut

!macro NSIS_HOOK_PREINSTALL
  StrCpy $LegacyUninstallString ""
  StrCpy $LegacyInstallDir ""
  StrCpy $CurrentUninstallString ""
  StrCpy $CurrentInstallDir ""
  StrCpy $MigrateDesktopShortcut 0

  ReadRegStr $LegacyUninstallString HKCU \
    "Software\Microsoft\Windows\CurrentVersion\Uninstall\自动点击流程台" \
    "UninstallString"
  ReadRegStr $LegacyInstallDir HKCU \
    "Software\Microsoft\Windows\CurrentVersion\Uninstall\自动点击流程台" \
    "InstallLocation"

  ${If} $LegacyInstallDir != ""
    StrCpy $R0 $LegacyInstallDir 1
    ${If} $R0 == '$\"'
      StrCpy $LegacyInstallDir $LegacyInstallDir "" 1
    ${EndIf}
    StrCpy $R0 $LegacyInstallDir 1 -1
    ${If} $R0 == '$\"'
      StrCpy $LegacyInstallDir $LegacyInstallDir -1
    ${EndIf}
  ${EndIf}

  IfFileExists "$DESKTOP\自动点击流程台.lnk" 0 +2
    StrCpy $MigrateDesktopShortcut 1
  IfFileExists "$DESKTOP\${PRODUCTNAME}.lnk" 0 +2
    StrCpy $MigrateDesktopShortcut 1

  ${If} $LegacyInstallDir != ""
    ReadRegStr $CurrentUninstallString HKCU \
      "Software\Microsoft\Windows\CurrentVersion\Uninstall\${PRODUCTNAME}" \
      "UninstallString"
    ReadRegStr $CurrentInstallDir HKCU \
      "Software\Microsoft\Windows\CurrentVersion\Uninstall\${PRODUCTNAME}" \
      "InstallLocation"

    ${If} $CurrentInstallDir != ""
      StrCpy $R0 $CurrentInstallDir 1
      ${If} $R0 == '$\"'
        StrCpy $CurrentInstallDir $CurrentInstallDir "" 1
      ${EndIf}
      StrCpy $R0 $CurrentInstallDir 1 -1
      ${If} $R0 == '$\"'
        StrCpy $CurrentInstallDir $CurrentInstallDir -1
      ${EndIf}
    ${EndIf}

    ${If} $CurrentUninstallString != ""
    ${AndIf} $CurrentInstallDir != $LegacyInstallDir
      DetailPrint "正在移除错误目录中的 ${PRODUCTNAME}"
      ExecWait '$CurrentUninstallString /S' $R0
    ${EndIf}

    ${If} $LegacyUninstallString != ""
      DetailPrint "正在迁移旧版自动点击流程台"
      ExecWait '$LegacyUninstallString /S' $R0
    ${EndIf}

    StrCpy $INSTDIR $LegacyInstallDir
    DetailPrint "新版将安装到原目录：$INSTDIR"
  ${EndIf}
!macroend

!macro NSIS_HOOK_POSTINSTALL
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
