; Windows installer for GlanceRiff.
; Build: makensis -NOCD -DVERSION=0.3.1 -DEXE=path\to\glanceriff.exe [-DOUTFILE=setup.exe] windows\installer.nsi

Unicode true
!include "MUI2.nsh"
!include "LogicLib.nsh"
!include "WinMessages.nsh"

!ifndef VERSION
  !error "Pass -DVERSION=X.Y.Z"
!endif
!ifndef EXE
  !error "Pass -DEXE=path\to\glanceriff.exe"
!endif
!ifndef OUTFILE
  !define OUTFILE "glanceriff-v${VERSION}-windows-x86_64-setup.exe"
!endif

!define UNINST_KEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\GlanceRiff"

Name "GlanceRiff ${VERSION}"
OutFile "${OUTFILE}"
InstallDir "$PROGRAMFILES64\GlanceRiff"
InstallDirRegKey HKLM "${UNINST_KEY}" "InstallLocation"
RequestExecutionLevel admin
SetCompressor /SOLID lzma

!define MUI_ICON "${__FILEDIR__}\glanceriff.ico"
!define MUI_UNICON "${__FILEDIR__}\glanceriff.ico"
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES
!insertmacro MUI_LANGUAGE "English"

VIProductVersion "${VERSION}.0"
VIAddVersionKey "ProductName" "GlanceRiff"
VIAddVersionKey "ProductVersion" "${VERSION}"
VIAddVersionKey "FileVersion" "${VERSION}"
VIAddVersionKey "FileDescription" "GlanceRiff Setup"

; RequestExecutionLevel only asks for elevation; this stops a run without admin rights (e.g. UAC turned off).
!macro RequireAdmin
  UserInfo::GetAccountType
  Pop $0
  ${If} $0 != "admin"
    MessageBox MB_ICONSTOP "GlanceRiff Setup must be run as administrator." /SD IDOK
    SetErrorLevel 740 ; ERROR_ELEVATION_REQUIRED
    Quit
  ${EndIf}
  SetRegView 64
!macroend

; Adds (ADD=1) or removes (ADD=0) $INSTDIR in the machine PATH.
; Done in PowerShell because NSIS strings are cut at 1024 characters, which would truncate a long PATH.
; The directory goes through an environment variable so quotes in the path can't break the command.
!macro SetPath ADD
  System::Call 'Kernel32::SetEnvironmentVariable(t "GLANCERIFF_DIR", t "$INSTDIR")i'
  nsExec::ExecToLog `powershell -NoProfile -NonInteractive -Command "$$k = [Microsoft.Win32.Registry]::LocalMachine.OpenSubKey('SYSTEM\CurrentControlSet\Control\Session Manager\Environment', $$true); $$d = $$env:GLANCERIFF_DIR; $$p = @($$k.GetValue('Path', '', 'DoNotExpandEnvironmentNames') -split ';' | Where-Object { $$_ -and $$_ -ne $$d }); if (${ADD}) { $$p += $$d }; $$k.SetValue('Path', ($$p -join ';'), 'ExpandString')"`
  Pop $0
  ${If} $0 != 0
    DetailPrint "Could not update PATH ($0)."
  ${EndIf}
  SendMessage ${HWND_BROADCAST} ${WM_SETTINGCHANGE} 0 "STR:Environment" /TIMEOUT=5000
!macroend

Function .onInit
  !insertmacro RequireAdmin
FunctionEnd

Function un.onInit
  !insertmacro RequireAdmin
FunctionEnd

Section
  SetOutPath "$INSTDIR"
  File "/oname=glanceriff.exe" "${EXE}"
  WriteUninstaller "$INSTDIR\uninstall.exe"

  SetShellVarContext all
  CreateShortcut "$SMPROGRAMS\GlanceRiff.lnk" "$INSTDIR\glanceriff.exe"
  CreateShortcut "$SMPROGRAMS\GlanceRiff Web.lnk" "$INSTDIR\glanceriff.exe" "--web"

  WriteRegStr HKLM "${UNINST_KEY}" "DisplayName" "GlanceRiff"
  WriteRegStr HKLM "${UNINST_KEY}" "DisplayVersion" "${VERSION}"
  WriteRegStr HKLM "${UNINST_KEY}" "Publisher" "SolidProgramming"
  WriteRegStr HKLM "${UNINST_KEY}" "URLInfoAbout" "https://github.com/SolidProgramming/GlanceRiff"
  WriteRegStr HKLM "${UNINST_KEY}" "InstallLocation" "$INSTDIR"
  WriteRegStr HKLM "${UNINST_KEY}" "DisplayIcon" "$INSTDIR\glanceriff.exe"
  WriteRegStr HKLM "${UNINST_KEY}" "UninstallString" '"$INSTDIR\uninstall.exe"'
  WriteRegStr HKLM "${UNINST_KEY}" "QuietUninstallString" '"$INSTDIR\uninstall.exe" /S'
  WriteRegDWORD HKLM "${UNINST_KEY}" "NoModify" 1
  WriteRegDWORD HKLM "${UNINST_KEY}" "NoRepair" 1

  !insertmacro SetPath 1
SectionEnd

Section "Uninstall"
  !insertmacro SetPath 0

  SetShellVarContext all
  Delete "$SMPROGRAMS\GlanceRiff.lnk"
  Delete "$SMPROGRAMS\GlanceRiff Web.lnk"

  Delete "$INSTDIR\glanceriff.exe"
  Delete "$INSTDIR\uninstall.exe"
  RMDir "$INSTDIR"

  DeleteRegKey HKLM "${UNINST_KEY}"
SectionEnd
