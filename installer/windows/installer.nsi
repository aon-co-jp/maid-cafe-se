; maid-cafe-se Windows installer (NSIS 3, Unicode). Builds maid-cafe-se-installer.exe.
;
; ビルド: installer\build-windows.ps1 が次のように呼ぶ / Called by installer\build-windows.ps1 as:
;   makensis /DVERSION=0.5.0 /DAPPDIR=<folder with maid-cafe-se.exe and .ico> /DOUTFILE=<installer.exe> installer.nsi
;
; 管理者権限は不要(ユーザー領域 %LOCALAPPDATA%\Programs\maid-cafe-se にインストール)。無人インストール: maid-cafe-se-installer.exe /S
; No admin rights needed (installs per-user). Silent install: maid-cafe-se-installer.exe /S

Unicode true
!include "MUI2.nsh"

!ifndef VERSION
  !define VERSION "0.0.0"
!endif
!ifndef APPDIR
  !error "APPDIR is required: /DAPPDIR=<folder containing maid-cafe-se.exe>"
!endif
!ifndef OUTFILE
  !define OUTFILE "maid-cafe-se-installer.exe"
!endif

!define APPNAME "maid-cafe-se"
!define EXENAME "maid-cafe-se.exe"
!define UNKEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\${APPNAME}"

Name "${APPNAME} ${VERSION}"
OutFile "${OUTFILE}"
RequestExecutionLevel user
InstallDir "$LOCALAPPDATA\Programs\${APPNAME}"
InstallDirRegKey HKCU "Software\${APPNAME}" "InstallDir"
SetCompressor /SOLID lzma
BrandingText "${APPNAME} ${VERSION}"

VIProductVersion "${VERSION}.0"
VIAddVersionKey "ProductName" "${APPNAME}"
VIAddVersionKey "ProductVersion" "${VERSION}"
VIAddVersionKey "FileVersion" "${VERSION}"
VIAddVersionKey "FileDescription" "${APPNAME} installer"
VIAddVersionKey "CompanyName" "aon-co-jp"

!define MUI_ICON "${APPDIR}\maid-cafe-se.ico"
!define MUI_UNICON "${APPDIR}\maid-cafe-se.ico"
!define MUI_ABORTWARNING
!define MUI_FINISHPAGE_RUN "$INSTDIR\${EXENAME}"
!define MUI_FINISHPAGE_RUN_TEXT "maid-cafe-se を起動する / Launch maid-cafe-se"

!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_PAGE_FINISH
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES

!insertmacro MUI_LANGUAGE "Japanese"
!insertmacro MUI_LANGUAGE "English"

Section "Install"
  ; 動作中なら止める(上書きインストール/アップデート時にファイルが掴まれているため)
  nsExec::Exec 'taskkill /F /IM ${EXENAME} /T'
  Pop $0
  Sleep 500

  SetOutPath "$INSTDIR"
  File /r "${APPDIR}\*.*"
  WriteUninstaller "$INSTDIR\uninstall.exe"

  CreateDirectory "$SMPROGRAMS\${APPNAME}"
  CreateShortcut "$SMPROGRAMS\${APPNAME}\${APPNAME}.lnk" "$INSTDIR\${EXENAME}"
  CreateShortcut "$SMPROGRAMS\${APPNAME}\Uninstall ${APPNAME}.lnk" "$INSTDIR\uninstall.exe"
  CreateShortcut "$DESKTOP\${APPNAME}.lnk" "$INSTDIR\${EXENAME}"

  WriteRegStr HKCU "Software\${APPNAME}" "InstallDir" "$INSTDIR"
  WriteRegStr HKCU "${UNKEY}" "DisplayName" "${APPNAME}"
  WriteRegStr HKCU "${UNKEY}" "DisplayVersion" "${VERSION}"
  WriteRegStr HKCU "${UNKEY}" "Publisher" "aon-co-jp"
  WriteRegStr HKCU "${UNKEY}" "DisplayIcon" "$INSTDIR\${EXENAME}"
  WriteRegStr HKCU "${UNKEY}" "InstallLocation" "$INSTDIR"
  WriteRegStr HKCU "${UNKEY}" "UninstallString" '"$INSTDIR\uninstall.exe"'
  WriteRegStr HKCU "${UNKEY}" "QuietUninstallString" '"$INSTDIR\uninstall.exe" /S'
  WriteRegDWORD HKCU "${UNKEY}" "NoModify" 1
  WriteRegDWORD HKCU "${UNKEY}" "NoRepair" 1
SectionEnd

Section "Uninstall"
  nsExec::Exec 'taskkill /F /IM ${EXENAME} /T'
  Pop $0
  Sleep 500

  ; アプリの自動起動の設定(v0.5.0以降はレジストリのRunキー、旧版はスタートアップのショートカット)も消す。アラーム設定(%APPDATA%\maid-cafe-se)はユーザーのデータなので残す。
  Delete "$SMSTARTUP\${APPNAME}.lnk"
  DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "${APPNAME}"
  Delete "$DESKTOP\${APPNAME}.lnk"
  RMDir /r "$SMPROGRAMS\${APPNAME}"
  RMDir /r "$INSTDIR"
  DeleteRegKey HKCU "${UNKEY}"
  DeleteRegKey HKCU "Software\${APPNAME}"
SectionEnd
