; Instalador do FeatherCode para Windows (NSIS). Gerado por pacote/gerar.sh:
;   makensis -DVERSAO=0.0.1 -DEXE=caminho/feathercode.exe -DSAIDA=caminho/instalador.exe pacote/feathercode.nsi
; Instala para todos os usuários em "Arquivos de Programas\FeatherWorks\FeatherCode" (pede
; administrador uma vez: só o administrador altera os .exe; a pasta FeatherWorks é da suíte), cria atalhos, entra no "Abrir com"
; dos arquivos de código e texto (sem virar o padrão de nenhum) e registra o desinstalador em "Aplicativos instalados".

Unicode true
!include "MUI2.nsh"

!ifndef VERSAO
  !define VERSAO "0.0.0"
!endif

Name "FeatherCode ${VERSAO}"
OutFile "${SAIDA}"
InstallDir "$PROGRAMFILES64\FeatherWorks\FeatherCode"
InstallDirRegKey HKLM "Software\FeatherWorks\FeatherCode" "Pasta"
RequestExecutionLevel admin
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
  WriteRegStr HKLM "Software\Classes\.${EXT}\OpenWithProgids" "${PROGID}" ""
!macroend
!macro TIRAR_ABRIR_COM EXT
  DeleteRegValue HKLM "Software\Classes\.${EXT}\OpenWithProgids" "${PROGID}"
!macroend

; Atalhos de todos os usuários e o registro de 64 bits (o programa é de 64 bits).
Function .onInit
  SetShellVarContext all
  SetRegView 64
FunctionEnd

Function un.onInit
  SetShellVarContext all
  SetRegView 64
FunctionEnd

Section "FeatherCode"
  ; As versões antigas instalavam só para o usuário (%LOCALAPPDATA%\Programs): tira essa cópia.
  ; _?= faz o desinstalador antigo rodar no lugar e esperar; o próprio .exe e a pasta saem aqui.
  ReadRegStr $2 HKCU "Software\FeatherWorks\FeatherCode" "Pasta"
  StrCmp $2 "" antiga_fora
  IfFileExists "$2\desinstalar.exe" 0 antiga_fora
    ExecWait '"$2\desinstalar.exe" /S _?=$2'
    Delete "$2\desinstalar.exe"
    RMDir "$2"
  antiga_fora:

  SetOutPath "$INSTDIR"
  ; Com o programa aberto, o .exe em uso não se sobrescreve, mas se renomeia; o .old sai
  ; na próxima instalação ou na desinstalação.
  Delete "$INSTDIR\feathercode.old"
  Rename "$INSTDIR\feathercode.exe" "$INSTDIR\feathercode.old"
  File "/oname=feathercode.exe" "${EXE}"
  File "icones\feathercode.ico"
  File "/oname=LICENSE.txt" "..\LICENSE"
  WriteUninstaller "$INSTDIR\desinstalar.exe"

  CreateShortCut "$SMPROGRAMS\FeatherCode.lnk" "$INSTDIR\feathercode.exe" "" "$INSTDIR\feathercode.ico"
  CreateShortCut "$DESKTOP\FeatherCode.lnk" "$INSTDIR\feathercode.exe" "" "$INSTDIR\feathercode.ico"

  ; Sem formato próprio: um ProgID só para o "Abrir com", sem tomar o padrão de extensão nenhuma.
  WriteRegStr HKLM "Software\Classes\${PROGID}" "" "Arquivo de código"
  WriteRegStr HKLM "Software\Classes\${PROGID}\DefaultIcon" "" "$INSTDIR\feathercode.ico"
  WriteRegStr HKLM "Software\Classes\${PROGID}\shell\open\command" "" '"$INSTDIR\feathercode.exe" "%1"'
  !insertmacro EXTENSOES ABRIR_COM

  WriteRegStr HKLM "Software\FeatherWorks\FeatherCode" "Pasta" "$INSTDIR"
  WriteRegStr HKLM "${DESINSTALAR}" "DisplayName" "FeatherCode"
  WriteRegStr HKLM "${DESINSTALAR}" "DisplayVersion" "${VERSAO}"
  WriteRegStr HKLM "${DESINSTALAR}" "Publisher" "FeatherWorks"
  ; Tamanho em "Programas e Recursos" (KB, do que foi instalado).
  ${GetSize} "$INSTDIR" "/S=0K" $0 $1 $2
  IntFmt $0 "0x%08X" $0
  WriteRegDWORD HKLM "${DESINSTALAR}" "EstimatedSize" "$0"
  WriteRegStr HKLM "${DESINSTALAR}" "DisplayIcon" "$INSTDIR\feathercode.ico"
  WriteRegStr HKLM "${DESINSTALAR}" "URLInfoAbout" "https://github.com/feather-works/feathercode"
  WriteRegStr HKLM "${DESINSTALAR}" "UninstallString" '"$INSTDIR\desinstalar.exe"'
  WriteRegStr HKLM "${DESINSTALAR}" "InstallLocation" "$INSTDIR"
  WriteRegDWORD HKLM "${DESINSTALAR}" "NoModify" 1
  WriteRegDWORD HKLM "${DESINSTALAR}" "NoRepair" 1
SectionEnd

Section "Uninstall"
  Delete "$SMPROGRAMS\FeatherCode.lnk"
  Delete "$DESKTOP\FeatherCode.lnk"
  Delete "$INSTDIR\feathercode.exe"
  Delete "$INSTDIR\feathercode.old"
  Delete "$INSTDIR\feathercode.ico"
  Delete "$INSTDIR\LICENSE.txt"
  Delete "$INSTDIR\desinstalar.exe"
  RMDir "$INSTDIR"
  ; A pasta da suíte só sai se ficou vazia (sem outro programa nem o Launcher).
  RMDir "$INSTDIR\.."
  DeleteRegKey HKLM "Software\Classes\${PROGID}"
  !insertmacro EXTENSOES TIRAR_ABRIR_COM
  DeleteRegKey HKLM "Software\FeatherWorks\FeatherCode"
  DeleteRegKey /ifempty HKLM "Software\FeatherWorks"
  DeleteRegKey HKLM "${DESINSTALAR}"
SectionEnd
