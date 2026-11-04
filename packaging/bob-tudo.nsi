Unicode true
!include "MUI2.nsh"
!include "LogicLib.nsh"
!include "FileFunc.nsh"

!ifndef EXE
  !define EXE "..\src-tauri\target\x86_64-pc-windows-msvc\release\bob-tudo.exe"
!endif
!ifndef ICON
  !define ICON "..\src-tauri\icons\icon.ico"
!endif

Name "bob tudo"
OutFile "bob-tudo-setup.exe"
InstallDir "$LOCALAPPDATA\bob tudo"
InstallDirRegKey HKCU "Software\bob tudo" "InstallDir"
RequestExecutionLevel user
ShowInstDetails show
ShowUninstDetails show

!define MUI_ICON "${ICON}"
!define MUI_UNICON "${ICON}"
!define MUI_ABORTWARNING

!define MUI_WELCOMEPAGE_TITLE "bob tudo"
!define MUI_WELCOMEPAGE_TEXT "este assistente instala o bob tudo para o usuário atual.$\r$\n$\r$\nlimpeza, discos, inicialização e aplicativos num programa só."
!define MUI_COMPONENTSPAGE_TITLE "componentes"
!define MUI_COMPONENTSPAGE_SUBTITLE "escolha o que instalar"
!define MUI_DIRECTORYPAGE_TEXT_TOP "escolha a pasta do bob tudo."
!define MUI_DIRECTORYPAGE_TEXT_DESTINATION "pasta"
!define MUI_FINISHPAGE_TITLE "o bob tudo foi instalado"
!define MUI_FINISHPAGE_TEXT "pode abrir o programa agora."
!define MUI_FINISHPAGE_RUN "$INSTDIR\bob-tudo.exe"
!define MUI_FINISHPAGE_RUN_TEXT "abrir o bob tudo"
!define MUI_UNCONFIRMPAGE_TEXT_TOP "o bob tudo será removido. os arquivos que você não pediu para apagar continuam no disco."

!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_COMPONENTS
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_PAGE_FINISH

!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES

!insertmacro MUI_LANGUAGE "PortugueseBR"
!insertmacro MUI_LANGUAGE "English"

Section "bob tudo" SecApp
  SectionIn RO
  SetOutPath "$INSTDIR"
  File "${EXE}"
  WriteUninstaller "$INSTDIR\Uninstall.exe"
  CreateDirectory "$SMPROGRAMS\bob tudo"
  CreateShortcut "$SMPROGRAMS\bob tudo\bob tudo.lnk" "$INSTDIR\bob-tudo.exe" "" "$INSTDIR\bob-tudo.exe" 0
  CreateShortcut "$SMPROGRAMS\bob tudo\desinstalar.lnk" "$INSTDIR\Uninstall.exe"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\bob tudo" "DisplayName" "bob tudo"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\bob tudo" "UninstallString" "$\"$INSTDIR\Uninstall.exe$\""
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\bob tudo" "DisplayIcon" "$INSTDIR\bob-tudo.exe"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\bob tudo" "Publisher" "goszc"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\bob tudo" "DisplayVersion" "0.2.0"
  WriteRegDWORD HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\bob tudo" "NoModify" 1
  WriteRegDWORD HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\bob tudo" "NoRepair" 1
  WriteRegStr HKCU "Software\bob tudo" "InstallDir" "$INSTDIR"
  ${GetSize} "$INSTDIR" "/S=0K" $0 $1 $2
  IntFmt $0 "0x%08X" $0
  WriteRegDWORD HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\bob tudo" "EstimatedSize" "$0"
SectionEnd

Section "atalho na área de trabalho" SecDesktop
  CreateShortcut "$DESKTOP\bob tudo.lnk" "$INSTDIR\bob-tudo.exe" "" "$INSTDIR\bob-tudo.exe" 0
SectionEnd

Section "Uninstall"
  Delete "$INSTDIR\bob-tudo.exe"
  Delete "$INSTDIR\Uninstall.exe"
  Delete "$DESKTOP\bob tudo.lnk"
  Delete "$SMPROGRAMS\bob tudo\bob tudo.lnk"
  Delete "$SMPROGRAMS\bob tudo\desinstalar.lnk"
  RMDir "$SMPROGRAMS\bob tudo"
  DeleteRegKey HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\bob tudo"
  DeleteRegKey HKCU "Software\bob tudo"
  RMDir "$INSTDIR"
SectionEnd

!insertmacro MUI_FUNCTION_DESCRIPTION_BEGIN
  !insertmacro MUI_DESCRIPTION_TEXT ${SecApp} "o programa e o atalho no menu iniciar"
  !insertmacro MUI_DESCRIPTION_TEXT ${SecDesktop} "cria um atalho na área de trabalho"
!insertmacro MUI_FUNCTION_DESCRIPTION_END
