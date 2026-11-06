; The Windows installer. Installs for the current user, so it needs no administrator rights.
; Built by packaging/docker/package.sh, which passes VERSION, EXE, ICON and OUT.

Unicode true
!include "MUI2.nsh"

Name "Pigeon"
OutFile "${OUT}"
InstallDir "$LOCALAPPDATA\Programs\Pigeon"
InstallDirRegKey HKCU "Software\Pigeon" "InstallDir"
RequestExecutionLevel user
SetCompressor /SOLID lzma

!define UNINSTALL_KEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\Pigeon"
!define MUI_ICON "${ICON}"
!define MUI_UNICON "${ICON}"
!define MUI_FINISHPAGE_RUN "$INSTDIR\pigeon.exe"
!define MUI_FINISHPAGE_RUN_TEXT "Open Pigeon"

!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_PAGE_FINISH
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES
!insertmacro MUI_LANGUAGE "English"

Section "Pigeon"
    SetOutPath "$INSTDIR"
    File "/oname=pigeon.exe" "${EXE}"
    File "/oname=FONT-LICENSES.txt" "${FONT_LICENSES}"
    WriteUninstaller "$INSTDIR\Uninstall Pigeon.exe"
    CreateShortcut "$SMPROGRAMS\Pigeon.lnk" "$INSTDIR\pigeon.exe"
    WriteRegStr HKCU "Software\Pigeon" "InstallDir" "$INSTDIR"
    WriteRegStr HKCU "${UNINSTALL_KEY}" "DisplayName" "Pigeon"
    WriteRegStr HKCU "${UNINSTALL_KEY}" "DisplayVersion" "${VERSION}"
    WriteRegStr HKCU "${UNINSTALL_KEY}" "DisplayIcon" "$INSTDIR\pigeon.exe"
    WriteRegStr HKCU "${UNINSTALL_KEY}" "Publisher" "Pigeon"
    WriteRegStr HKCU "${UNINSTALL_KEY}" "UninstallString" '"$INSTDIR\Uninstall Pigeon.exe"'
    WriteRegDWORD HKCU "${UNINSTALL_KEY}" "NoModify" 1
    WriteRegDWORD HKCU "${UNINSTALL_KEY}" "NoRepair" 1
SectionEnd

; Removes the program only. Workspaces, settings and history stay.
Section "Uninstall"
    Delete "$SMPROGRAMS\Pigeon.lnk"
    Delete "$INSTDIR\pigeon.exe"
    Delete "$INSTDIR\FONT-LICENSES.txt"
    Delete "$INSTDIR\Uninstall Pigeon.exe"
    RMDir "$INSTDIR"
    DeleteRegKey HKCU "${UNINSTALL_KEY}"
    DeleteRegKey HKCU "Software\Pigeon"
SectionEnd
