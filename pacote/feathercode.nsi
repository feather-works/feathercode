; Instalador do FeatherCode para Windows (NSIS). Gerado por pacote/gerar.sh:
;   makensis -DVERSAO=0.0.1 -DEXE=caminho/feathercode.exe -DSAIDA=caminho/instalador.exe pacote/feathercode.nsi
; Instala por usuário (sem pedir administrador), cria atalhos, entra no "Abrir com"
; dos arquivos de código e texto (sem virar o padrão de nenhum) e registra o desinstalador em "Aplicativos instalados".

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

!include "FileFunc.nsh"
!define DESINSTALAR "Software\Microsoft\Windows\CurrentVersion\Uninstall\FeatherCode"
!define PROGID "FeatherCode.Arquivo"

; Extensões em que o FeatherCode aparece no "Abrir com" (a mesma lista instala e desinstala).
!macro EXTENSOES ACAO
  !insertmacro ${ACAO} rs
  !insertmacro ${ACAO} py
  !insertmacro ${ACAO} js
  !insertmacro ${ACAO} ts
  !insertmacro ${ACAO} jsx
  !insertmacro ${ACAO} tsx
  !insertmacro ${ACAO} json
  !insertmacro ${ACAO} toml
  !insertmacro ${ACAO} yaml
  !insertmacro ${ACAO} yml
  !insertmacro ${ACAO} md
  !insertmacro ${ACAO} txt
  !insertmacro ${ACAO} log
  !insertmacro ${ACAO} ini
  !insertmacro ${ACAO} cfg
  !insertmacro ${ACAO} xml
  !insertmacro ${ACAO} html
  !insertmacro ${ACAO} css
  !insertmacro ${ACAO} c
  !insertmacro ${ACAO} h
  !insertmacro ${ACAO} cpp
  !insertmacro ${ACAO} hpp
  !insertmacro ${ACAO} cs
  !insertmacro ${ACAO} go
  !insertmacro ${ACAO} java
  !insertmacro ${ACAO} kt
  !insertmacro ${ACAO} lua
  !insertmacro ${ACAO} rb
  !insertmacro ${ACAO} php
  !insertmacro ${ACAO} sql
  !insertmacro ${ACAO} sh
  !insertmacro ${ACAO} ps1
!macroend
!macro ABRIR_COM EXT
  WriteRegStr HKCU "Software\Classes\.${EXT}\OpenWithProgids" "${PROGID}" ""
!macroend
!macro TIRAR_ABRIR_COM EXT
  DeleteRegValue HKCU "Software\Classes\.${EXT}\OpenWithProgids" "${PROGID}"
!macroend

Section "FeatherCode"
  SetOutPath "$INSTDIR"
  File "/oname=feathercode.exe" "${EXE}"
  File "icones\feathercode.ico"
  File "/oname=LICENSE.txt" "..\LICENSE"
  WriteUninstaller "$INSTDIR\desinstalar.exe"

  CreateShortCut "$SMPROGRAMS\FeatherCode.lnk" "$INSTDIR\feathercode.exe" "" "$INSTDIR\feathercode.ico"
  CreateShortCut "$DESKTOP\FeatherCode.lnk" "$INSTDIR\feathercode.exe" "" "$INSTDIR\feathercode.ico"

  ; Sem formato próprio: um ProgID só para o "Abrir com", sem tomar o padrão de extensão nenhuma.
  WriteRegStr HKCU "Software\Classes\${PROGID}" "" "Arquivo de código"
  WriteRegStr HKCU "Software\Classes\${PROGID}\DefaultIcon" "" "$INSTDIR\feathercode.ico"
  WriteRegStr HKCU "Software\Classes\${PROGID}\shell\open\command" "" '"$INSTDIR\feathercode.exe" "%1"'
  !insertmacro EXTENSOES ABRIR_COM

  WriteRegStr HKCU "Software\FeatherWorks\FeatherCode" "Pasta" "$INSTDIR"
  WriteRegStr HKCU "${DESINSTALAR}" "DisplayName" "FeatherCode"
  WriteRegStr HKCU "${DESINSTALAR}" "DisplayVersion" "${VERSAO}"
  WriteRegStr HKCU "${DESINSTALAR}" "Publisher" "FeatherWorks"
  ; Tamanho em "Programas e Recursos" (KB, do que foi instalado).
  ${GetSize} "$INSTDIR" "/S=0K" $0 $1 $2
  IntFmt $0 "0x%08X" $0
  WriteRegDWORD HKCU "${DESINSTALAR}" "EstimatedSize" "$0"
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
  DeleteRegKey HKCU "Software\Classes\${PROGID}"
  !insertmacro EXTENSOES TIRAR_ABRIR_COM
  DeleteRegKey HKCU "Software\FeatherWorks\FeatherCode"
  DeleteRegKey HKCU "${DESINSTALAR}"
SectionEnd
