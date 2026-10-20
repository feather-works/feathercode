; Instalador do FeatherCode para Windows (NSIS). Gerado por pacote/gerar.sh:
;   makensis -DVERSAO=0.0.1 -DEXE=caminho/feathercode.exe -DSAIDA=caminho/instalador.exe pacote/feathercode.nsi
; Instala por usuário (sem pedir administrador), cria atalhos
; e registra o desinstalador em "Aplicativos instalados".

Unicode true
!include "MUI2.nsh"

!ifndef VERSAO
  !define VERSAO "0.0.0"
!endif

Name "FeatherCode ${VERSAO}"
OutFile "${SAIDA}"
InstallDir "$LOCALAPPDATA\Programs\FeatherCode"
InstallDirRegKey HKCU "Software\FeatherWorks\FeatherCode" "Pasta"
RequestExecutionLevel user
SetCompressor /SOLID lzma

!define MUI_ICON "icones\feathercode.ico"
!define MUI_UNICON "icones\feathercode.ico"
!define MUI_FINISHPAGE_RUN "$INSTDIR\feathercode.exe"
!define MUI_FINISHPAGE_RUN_TEXT "Abrir o FeatherCode"

!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_LICENSE "..\LICENSE"
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_PAGE_FINISH
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES
!insertmacro MUI_LANGUAGE "PortugueseBR"

!define DESINSTALAR "Software\Microsoft\Windows\CurrentVersion\Uninstall\FeatherCode"

Section "FeatherCode"
  SetOutPath "$INSTDIR"
  File "/oname=feathercode.exe" "${EXE}"
  File "icones\feathercode.ico"
  File "/oname=LICENSE.txt" "..\LICENSE"
  WriteUninstaller "$INSTDIR\desinstalar.exe"

  CreateShortCut "$SMPROGRAMS\FeatherCode.lnk" "$INSTDIR\feathercode.exe" "" "$INSTDIR\feathercode.ico"
  CreateShortCut "$DESKTOP\FeatherCode.lnk" "$INSTDIR\feathercode.exe" "" "$INSTDIR\feathercode.ico"

  WriteRegStr HKCU "Software\FeatherWorks\FeatherCode" "Pasta" "$INSTDIR"
  WriteRegStr HKCU "${DESINSTALAR}" "DisplayName" "FeatherCode"
  WriteRegStr HKCU "${DESINSTALAR}" "DisplayVersion" "${VERSAO}"
  WriteRegStr HKCU "${DESINSTALAR}" "Publisher" "FeatherWorks"
  WriteRegStr HKCU "${DESINSTALAR}" "DisplayIcon" "$INSTDIR\feathercode.ico"
  WriteRegStr HKCU "${DESINSTALAR}" "URLInfoAbout" "https://github.com/feather-works/feathercode"
  WriteRegStr HKCU "${DESINSTALAR}" "UninstallString" '"$INSTDIR\desinstalar.exe"'
  WriteRegDWORD HKCU "${DESINSTALAR}" "NoModify" 1
  WriteRegDWORD HKCU "${DESINSTALAR}" "NoRepair" 1
SectionEnd

Section "Uninstall"
  Delete "$SMPROGRAMS\FeatherCode.lnk"
  Delete "$DESKTOP\FeatherCode.lnk"
  Delete "$INSTDIR\feathercode.exe"
  Delete "$INSTDIR\feathercode.ico"
  Delete "$INSTDIR\LICENSE.txt"
  Delete "$INSTDIR\desinstalar.exe"
  RMDir "$INSTDIR"
  DeleteRegKey HKCU "Software\FeatherWorks\FeatherCode"
  DeleteRegKey HKCU "${DESINSTALAR}"
SectionEnd
