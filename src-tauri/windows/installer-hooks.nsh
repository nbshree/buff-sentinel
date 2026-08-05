Var LegacyInstallDir
Var PreviousInstallDir
Var IncorrectDefaultInstallDir
Var MigrateDesktopShortcut

!macro NSIS_HOOK_PREINSTALL
  StrCpy $LegacyInstallDir ""
  StrCpy $PreviousInstallDir ""
  StrCpy $MigrateDesktopShortcut 0

  ReadRegStr $LegacyInstallDir HKCU \
    "Software\Microsoft\Windows\CurrentVersion\Uninstall\自动点击流程台" \
    "InstallLocation"
  ReadRegStr $PreviousInstallDir HKCU \
    "Software\Microsoft\Windows\CurrentVersion\Uninstall\${PRODUCTNAME}" \
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

  ${If} $PreviousInstallDir != ""
    StrCpy $R0 $PreviousInstallDir 1
    ${If} $R0 == '$\"'
      StrCpy $PreviousInstallDir $PreviousInstallDir "" 1
    ${EndIf}
    StrCpy $R0 $PreviousInstallDir 1 -1
    ${If} $R0 == '$\"'
      StrCpy $PreviousInstallDir $PreviousInstallDir -1
    ${EndIf}
  ${EndIf}

  IfFileExists "$DESKTOP\自动点击流程台.lnk" 0 +2
    StrCpy $MigrateDesktopShortcut 1
  IfFileExists "$DESKTOP\${PRODUCTNAME}.lnk" 0 +2
    StrCpy $MigrateDesktopShortcut 1

  ; Never run either old uninstaller here. An NSIS uninstaller can return before its
  ; cleanup process finishes and delete files copied by the new installer afterwards.
  ${If} $LegacyInstallDir != ""
    StrCpy $INSTDIR $LegacyInstallDir
    DetailPrint "新版将安装到原目录：$INSTDIR"
  ${ElseIf} $PreviousInstallDir != ""
    StrCpy $INSTDIR $PreviousInstallDir
  ${EndIf}

  ; Tauri calls SetOutPath before this hook. Keep NSIS' output directory in sync
  ; after changing $INSTDIR, otherwise the first migrated update copies the EXE to
  ; the old default directory while registry entries point at the legacy directory.
  SetOutPath "$INSTDIR"
!macroend

!macro NSIS_HOOK_POSTINSTALL
  ; The new installer now owns the destination. Remove only stale registration and
  ; known executable files from incorrect directories; do not run an old uninstaller.
  DeleteRegKey HKCU \
    "Software\Microsoft\Windows\CurrentVersion\Uninstall\自动点击流程台"

  ${If} $PreviousInstallDir != ""
  ${AndIf} $PreviousInstallDir != $INSTDIR
    Delete /REBOOTOK "$PreviousInstallDir\${MAINBINARYNAME}.exe"
    Delete /REBOOTOK "$PreviousInstallDir\uninstall.exe"
    RMDir "$PreviousInstallDir"
  ${EndIf}

  ; v2.0.0-v2.0.2 could leave the executable in this default directory while the
  ; registry and shortcut pointed elsewhere. Clean that exact known location too.
  StrCpy $IncorrectDefaultInstallDir "$LOCALAPPDATA\${PRODUCTNAME}"
  ${If} $IncorrectDefaultInstallDir != $INSTDIR
    Delete /REBOOTOK "$IncorrectDefaultInstallDir\${MAINBINARYNAME}.exe"
    Delete /REBOOTOK "$IncorrectDefaultInstallDir\uninstall.exe"
    RMDir "$IncorrectDefaultInstallDir"
  ${EndIf}

  Delete "$DESKTOP\自动点击流程台.lnk"
  Delete "$DESKTOP\${PRODUCTNAME}.lnk"
  Delete "$SMPROGRAMS\自动点击流程台.lnk"
  Delete "$SMPROGRAMS\自动点击流程台\自动点击流程台.lnk"
  RMDir "$SMPROGRAMS\自动点击流程台"

  SetOutPath "$INSTDIR"
  CreateDirectory "$SMPROGRAMS\${STARTMENUFOLDER}"
  Delete "$SMPROGRAMS\${STARTMENUFOLDER}\${PRODUCTNAME}.lnk"
  CreateShortcut \
    "$SMPROGRAMS\${STARTMENUFOLDER}\${PRODUCTNAME}.lnk" \
    "$INSTDIR\${MAINBINARYNAME}.exe" \
    "" \
    "$INSTDIR\${MAINBINARYNAME}.exe" \
    0

  ${If} $MigrateDesktopShortcut = 1
    CreateShortcut \
      "$DESKTOP\${PRODUCTNAME}.lnk" \
      "$INSTDIR\${MAINBINARYNAME}.exe" \
      "" \
      "$INSTDIR\${MAINBINARYNAME}.exe" \
      0
  ${EndIf}
!macroend
