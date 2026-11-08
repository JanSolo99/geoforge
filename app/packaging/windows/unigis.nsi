; setup.exe installer (NSIS 3), built by package.sh. A per-user install that needs no admin
; rights, in %LOCALAPPDATA%\Programs\UniGIS: a Start menu shortcut, ug on the user's
; PATH, .rgis/.rgisx projects opened by UniGIS and UniGIS offered under Open with for .geojson.
; Defines passed with -D: VERSION, VERSION4 (four numbers), STAGE (the folder holding the exes,
; LICENSE and path.ps1), ICON, SIDEBAR (a 164x314 BMP) and OUTFILE.

Unicode true
SetCompressor /SOLID lzma
RequestExecutionLevel user
ManifestDPIAware true

!define APP "UniGIS"
!define PUBLISHER "Open Geospatial Solutions"
!define UNINST_KEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\${APP}"

Name "UniGIS"
OutFile "${OUTFILE}"
InstallDir "$LOCALAPPDATA\Programs\${APP}"
InstallDirRegKey HKCU "Software\${APP}" "InstallDir"

VIProductVersion "${VERSION4}"
VIAddVersionKey "ProductName" "UniGIS"
VIAddVersionKey "ProductVersion" "${VERSION}"
VIAddVersionKey "FileVersion" "${VERSION}"
VIAddVersionKey "FileDescription" "UniGIS ${VERSION} setup"
VIAddVersionKey "CompanyName" "${PUBLISHER}"
VIAddVersionKey "LegalCopyright" "Copyright the UniGIS contributors. Apache-2.0."

!include "MUI2.nsh"
!include "WinMessages.nsh"

!define MUI_ICON "${ICON}"
!define MUI_UNICON "${ICON}"
!define MUI_WELCOMEFINISHPAGE_BITMAP "${SIDEBAR}"
!define MUI_UNWELCOMEFINISHPAGE_BITMAP "${SIDEBAR}"
!define MUI_ABORTWARNING
!define MUI_FINISHPAGE_RUN "$INSTDIR\unigis.exe"
!define MUI_FINISHPAGE_RUN_TEXT "Start UniGIS"

!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_LICENSE "${STAGE}\LICENSE"
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_PAGE_FINISH

!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES

!insertmacro MUI_LANGUAGE "English"

; Asks Explorer to pick up changed file associations.
!macro RefreshShell
  System::Call 'shell32::SHChangeNotify(i 0x08000000, i 0, p 0, p 0)'
!macroend

; Tells running programs (a new terminal) that the user's PATH changed.
!macro RefreshEnvironment
  SendMessage ${HWND_BROADCAST} ${WM_SETTINGCHANGE} 0 "STR:Environment" /TIMEOUT=5000
!macroend

Section "Install"
  SetOutPath "$INSTDIR"
  File "${STAGE}\unigis.exe"
  File "${STAGE}\unigis-cli.exe"
  File "${STAGE}\ug.exe"
  File /oname=LICENSE.txt "${STAGE}\LICENSE"
  WriteUninstaller "$INSTDIR\uninstall.exe"

  CreateShortcut "$SMPROGRAMS\UniGIS.lnk" "$INSTDIR\unigis.exe"

  ; ug from any new terminal.
  InitPluginsDir
  File /oname=$PLUGINSDIR\path.ps1 "${STAGE}\path.ps1"
  nsExec::ExecToLog 'powershell.exe -NoProfile -NonInteractive -ExecutionPolicy Bypass -File "$PLUGINSDIR\path.ps1" add "$INSTDIR"'
  Pop $0
  StrCmp $0 0 +2
    MessageBox MB_OK|MB_ICONEXCLAMATION "UniGIS is installed, but ug could not be added to your PATH ($0). Run it from $INSTDIR instead." /SD IDOK
  !insertmacro RefreshEnvironment

  ; Projects open in UniGIS; GeoJSON lists it under Open with.
  WriteRegStr HKCU "Software\Classes\UniGIS.Project" "" "UniGIS project"
  WriteRegStr HKCU "Software\Classes\UniGIS.Project\DefaultIcon" "" '"$INSTDIR\unigis.exe",0'
  WriteRegStr HKCU "Software\Classes\UniGIS.Project\shell\open\command" "" '"$INSTDIR\unigis.exe" "%1"'
  WriteRegStr HKCU "Software\Classes\.rgis" "" "UniGIS.Project"
  WriteRegStr HKCU "Software\Classes\.rgisx" "" "UniGIS.Project"
  WriteRegStr HKCU "Software\Classes\.geojson\OpenWithProgids" "UniGIS.Project" ""
  !insertmacro RefreshShell

  ; Apps & features.
  WriteRegStr HKCU "Software\${APP}" "InstallDir" "$INSTDIR"
  WriteRegStr HKCU "${UNINST_KEY}" "DisplayName" "UniGIS"
  WriteRegStr HKCU "${UNINST_KEY}" "DisplayVersion" "${VERSION}"
  WriteRegStr HKCU "${UNINST_KEY}" "Publisher" "${PUBLISHER}"
  WriteRegStr HKCU "${UNINST_KEY}" "DisplayIcon" '"$INSTDIR\unigis.exe",0'
  WriteRegStr HKCU "${UNINST_KEY}" "InstallLocation" "$INSTDIR"
  WriteRegStr HKCU "${UNINST_KEY}" "URLInfoAbout" "https://rustgis.app"
  WriteRegStr HKCU "${UNINST_KEY}" "UninstallString" '"$INSTDIR\uninstall.exe"'
  WriteRegStr HKCU "${UNINST_KEY}" "QuietUninstallString" '"$INSTDIR\uninstall.exe" /S'
  WriteRegDWORD HKCU "${UNINST_KEY}" "NoModify" 1
  WriteRegDWORD HKCU "${UNINST_KEY}" "NoRepair" 1
  SectionGetSize 0 $0
  WriteRegDWORD HKCU "${UNINST_KEY}" "EstimatedSize" $0
SectionEnd

Section "Uninstall"
  Delete "$SMPROGRAMS\UniGIS.lnk"

  InitPluginsDir
  File /oname=$PLUGINSDIR\path.ps1 "${STAGE}\path.ps1"
  nsExec::ExecToLog 'powershell.exe -NoProfile -NonInteractive -ExecutionPolicy Bypass -File "$PLUGINSDIR\path.ps1" remove "$INSTDIR"'
  Pop $0
  StrCmp $0 0 +2
    MessageBox MB_OK|MB_ICONEXCLAMATION "$INSTDIR could not be removed from your PATH ($0). Remove it in Settings › System › About › Advanced system settings › Environment Variables." /SD IDOK
  !insertmacro RefreshEnvironment

  ; Only the associations this installer made: another program may own .geojson.
  DeleteRegValue HKCU "Software\Classes\.geojson\OpenWithProgids" "UniGIS.Project"
  ReadRegStr $0 HKCU "Software\Classes\.rgis" ""
  StrCmp $0 "UniGIS.Project" 0 +2
    DeleteRegKey HKCU "Software\Classes\.rgis"
  ReadRegStr $0 HKCU "Software\Classes\.rgisx" ""
  StrCmp $0 "UniGIS.Project" 0 +2
    DeleteRegKey HKCU "Software\Classes\.rgisx"
  DeleteRegKey HKCU "Software\Classes\UniGIS.Project"
  !insertmacro RefreshShell

  Delete "$INSTDIR\unigis.exe"
  Delete "$INSTDIR\unigis-cli.exe"
  Delete "$INSTDIR\ug.exe"
  Delete "$INSTDIR\LICENSE.txt"
  Delete "$INSTDIR\uninstall.exe"
  RMDir "$INSTDIR"

  DeleteRegKey HKCU "${UNINST_KEY}"
  DeleteRegValue HKCU "Software\${APP}" "InstallDir"
  DeleteRegKey /ifempty HKCU "Software\${APP}"
SectionEnd
