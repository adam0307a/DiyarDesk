Unicode true
!include "MUI2.nsh"
!include "LogicLib.nsh"
!include "x64.nsh"
!include "WinVer.nsh"
!include "FileFunc.nsh"
!include "StrFunc.nsh"
${UnStrStr}
Name "DiyarDesk"
OutFile "${OUTPUT}"
InstallDir "$LOCALAPPDATA\Programs\DiyarDesk"
InstallDirRegKey HKCU "Software\DesklandPreview\Installer" "InstallDir"
RequestExecutionLevel user
SetCompressor /SOLID lzma
BrandingText "DiyarDesk"
VIProductVersion "${VERSION}.0"
VIAddVersionKey /LANG=1033 "ProductName" "DiyarDesk"
VIAddVersionKey /LANG=1033 "FileDescription" "DiyarDesk Installer"
VIAddVersionKey /LANG=1033 "FileVersion" "${VERSION}"
VIAddVersionKey /LANG=1033 "ProductVersion" "${VERSION}"
VIAddVersionKey /LANG=1033 "LegalCopyright" "Copyright 2026 DiyarDesk & Deskland authors"
!define MUI_ABORTWARNING
!define MUI_ICON "${PAYLOAD}\host\bin\DesklandPreview.ico"
!define MUI_UNICON "${PAYLOAD}\host\bin\DesklandPreview.ico"
!define MUI_FINISHPAGE_RUN "$INSTDIR\host\bin\Deskland.exe"
!define MUI_FINISHPAGE_RUN_TEXT "$(LaunchDeskland)"
!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_LICENSE "${PAYLOAD}\LICENSE"
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_PAGE_FINISH
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES
!insertmacro MUI_UNPAGE_FINISH
!insertmacro MUI_LANGUAGE "English"
!insertmacro MUI_LANGUAGE "Turkish"
!insertmacro MUI_LANGUAGE "SimpChinese"
!insertmacro MUI_LANGUAGE "TradChinese"
!insertmacro MUI_LANGUAGE "German"
!insertmacro MUI_LANGUAGE "French"
!insertmacro MUI_LANGUAGE "Japanese"
!insertmacro MUI_LANGUAGE "Korean"

LangString LaunchDeskland ${LANG_ENGLISH} "Launch DiyarDesk"
LangString LaunchDeskland ${LANG_TURKISH} "DiyarDesk'i Başlat"
LangString LaunchDeskland ${LANG_SIMPCHINESE} "启动 DiyarDesk"
LangString LaunchDeskland ${LANG_TRADCHINESE} "啟動 DiyarDesk"
LangString LaunchDeskland ${LANG_GERMAN} "DiyarDesk starten"
LangString LaunchDeskland ${LANG_FRENCH} "Lancer DiyarDesk"
LangString LaunchDeskland ${LANG_JAPANESE} "DiyarDesk を起動"
LangString LaunchDeskland ${LANG_KOREAN} "DiyarDesk 실행"

LangString AppFiles ${LANG_ENGLISH} "DiyarDesk application and themes"
LangString AppFiles ${LANG_TURKISH} "DiyarDesk uygulaması ve temalar"
LangString AppFiles ${LANG_SIMPCHINESE} "DiyarDesk 主程序与主题"
LangString AppFiles ${LANG_TRADCHINESE} "DiyarDesk 主程式與主題"
LangString AppFiles ${LANG_GERMAN} "DiyarDesk-Anwendung und Themen"
LangString AppFiles ${LANG_FRENCH} "Application DiyarDesk et thèmes"
LangString AppFiles ${LANG_JAPANESE} "DiyarDesk アプリとテーマ"
LangString AppFiles ${LANG_KOREAN} "DiyarDesk 앱 및 테마"

LangString DesktopShortcut ${LANG_ENGLISH} "Desktop shortcut"
LangString DesktopShortcut ${LANG_TURKISH} "Masaüstü kısayolu"
LangString DesktopShortcut ${LANG_SIMPCHINESE} "桌面快捷方式"
LangString DesktopShortcut ${LANG_TRADCHINESE} "桌面捷徑"
LangString DesktopShortcut ${LANG_GERMAN} "Desktop-Verknüpfung"
LangString DesktopShortcut ${LANG_FRENCH} "Raccourci sur le bureau"
LangString DesktopShortcut ${LANG_JAPANESE} "デスクトップショートカット"
LangString DesktopShortcut ${LANG_KOREAN} "바탕 화면 바로 가기"

LangString NeedX64 ${LANG_ENGLISH} "64-bit Windows 10 or later is required."
LangString NeedX64 ${LANG_TURKISH} "64-bit Windows 10 veya üzeri gereklidir."
LangString NeedX64 ${LANG_SIMPCHINESE} "需要 64 位 Windows 10 或更高版本。"
LangString NeedX64 ${LANG_TRADCHINESE} "需要 64 位元 Windows 10 或更新版本。"
LangString NeedX64 ${LANG_GERMAN} "64-Bit-Windows 10 oder höher ist erforderlich."
LangString NeedX64 ${LANG_FRENCH} "Windows 10 64 bits ou version ultérieure est requis."
LangString NeedX64 ${LANG_JAPANESE} "64ビット版 Windows 10 以降が必要です。"
LangString NeedX64 ${LANG_KOREAN} "64비트 Windows 10 이상이 필요합니다."

LangString CloseFailed ${LANG_ENGLISH} "Please exit DiyarDesk and retry."
LangString CloseFailed ${LANG_TURKISH} "Lütfen DiyarDesk'ten çıkın ve yeniden deneyin."
LangString CloseFailed ${LANG_SIMPCHINESE} "请退出正在运行的 DiyarDesk 后重试。"
LangString CloseFailed ${LANG_TRADCHINESE} "請先結束執行中的 DiyarDesk 後再重試。"
LangString CloseFailed ${LANG_GERMAN} "Bitte beenden Sie DiyarDesk und versuchen Sie es erneut."
LangString CloseFailed ${LANG_FRENCH} "Veuillez quitter DiyarDesk puis réessayer."
LangString CloseFailed ${LANG_JAPANESE} "実行中の DiyarDesk を終了してから再試行してください。"
LangString CloseFailed ${LANG_KOREAN} "실행 중인 DiyarDesk를 종료한 후 다시 시도하세요."

LangString RuntimeFailed ${LANG_ENGLISH} "WebView2 setup did not complete. Connect to the internet, install Microsoft Edge WebView2 Runtime, and retry."
LangString RuntimeFailed ${LANG_TURKISH} "WebView2 kurulumu tamamlanamadı. İnternete bağlanın, Microsoft Edge WebView2 Runtime'ı yükleyin ve yeniden deneyin."
LangString RuntimeFailed ${LANG_SIMPCHINESE} "WebView2 安装未完成。请连接网络，安装 Microsoft Edge WebView2 Runtime 后重试。"
LangString RuntimeFailed ${LANG_TRADCHINESE} "WebView2 安裝未完成。請連接網路，安裝 Microsoft Edge WebView2 Runtime 後再重試。"
LangString RuntimeFailed ${LANG_GERMAN} "Die WebView2-Installation wurde nicht abgeschlossen. Stellen Sie eine Internetverbindung her, installieren Sie Microsoft Edge WebView2 Runtime und versuchen Sie es erneut."
LangString RuntimeFailed ${LANG_FRENCH} "L'installation de WebView2 n'a pas pu aboutir. Connectez-vous à Internet, installez Microsoft Edge WebView2 Runtime et réessayez."
LangString RuntimeFailed ${LANG_JAPANESE} "WebView2 のセットアップが完了しませんでした。インターネットに接続し、Microsoft Edge WebView2 Runtime をインストールして再試行してください。"
LangString RuntimeFailed ${LANG_KOREAN} "WebView2 설치가 완료되지 않았습니다. 인터넷에 연결하여 Microsoft Edge WebView2 Runtime을 설치한 후 다시 시도하세요."

LangString InstallingRuntime ${LANG_ENGLISH} "Installing Microsoft Edge WebView2 Runtime (internet required)..."
LangString InstallingRuntime ${LANG_TURKISH} "Microsoft Edge WebView2 Runtime yükleniyor (internet gerekir)..."
LangString InstallingRuntime ${LANG_SIMPCHINESE} "正在安装 Microsoft Edge WebView2 Runtime（需要联网）..."
LangString InstallingRuntime ${LANG_TRADCHINESE} "正在安裝 Microsoft Edge WebView2 Runtime（需要網際網路）..."
LangString InstallingRuntime ${LANG_GERMAN} "Microsoft Edge WebView2 Runtime wird installiert (Internet erforderlich)..."
LangString InstallingRuntime ${LANG_FRENCH} "Installation de Microsoft Edge WebView2 Runtime (connexion Internet requise)..."
LangString InstallingRuntime ${LANG_JAPANESE} "Microsoft Edge WebView2 Runtime をインストールしています（インターネット接続が必要）..."
LangString InstallingRuntime ${LANG_KOREAN} "Microsoft Edge WebView2 Runtime 설치 중(인터넷 필요)..."

Function .onInit
  ${IfNot} ${RunningX64}
    MessageBox MB_ICONSTOP "$(NeedX64)"
    SetErrorLevel 1
    Abort
  ${EndIf}
  ${IfNot} ${AtLeastWin10}
    MessageBox MB_ICONSTOP "$(NeedX64)"
    SetErrorLevel 1
    Abort
  ${EndIf}
  SetShellVarContext current
  SetRegView 64
  !insertmacro MUI_LANGDLL_DISPLAY
FunctionEnd

Function CheckRuntime
  StrCpy $0 ""
  SetRegView 32
  ReadRegStr $0 HKLM "Software\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}" "pv"
  ${If} $0 == ""
    ReadRegStr $0 HKCU "Software\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}" "pv"
  ${EndIf}
  SetRegView 64
  ${If} $0 == ""
    ReadRegStr $0 HKCU "Software\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}" "pv"
  ${EndIf}
FunctionEnd

Section "$(AppFiles)" SecApp
  SectionIn RO
  Call CheckRuntime
  ${If} $0 == ""
  ${OrIf} $0 == "0.0.0.0"
    InitPluginsDir
    SetOutPath "$PLUGINSDIR"
    File /oname=MicrosoftEdgeWebview2Setup.exe "${BOOTSTRAPPER}"
    DetailPrint "$(InstallingRuntime)"
    ExecWait '"$PLUGINSDIR\MicrosoftEdgeWebview2Setup.exe" /silent /install' $1
    Call CheckRuntime
    ${If} $0 == ""
    ${OrIf} $0 == "0.0.0.0"
      MessageBox MB_ICONSTOP "$(RuntimeFailed)"
      SetErrorLevel 2
      Abort
    ${EndIf}
  ${EndIf}
  IfFileExists "$INSTDIR\host\bin\Deskland.exe" 0 closed
  ExecWait '"$INSTDIR\host\bin\Deskland.exe" --exit-installed' $0
  ${If} $0 != 0
    MessageBox MB_ICONSTOP "$(CloseFailed)"
    SetErrorLevel 3
    Abort
  ${EndIf}
  closed:
  !include "${FILE_LIST}"
  WriteUninstaller "$INSTDIR\Uninstall.exe"
  CreateDirectory "$SMPROGRAMS\DiyarDesk"
  CreateShortcut "$SMPROGRAMS\DiyarDesk\DiyarDesk.lnk" "$INSTDIR\host\bin\Deskland.exe"
  CreateShortcut "$SMPROGRAMS\DiyarDesk\Uninstall.lnk" "$INSTDIR\Uninstall.exe"
  WriteRegStr HKCU "Software\DesklandPreview\Installer" "InstallDir" "$INSTDIR"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\DesklandPreview" "DisplayName" "DiyarDesk"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\DesklandPreview" "DisplayVersion" "${VERSION}"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\DesklandPreview" "Publisher" "DiyarDesk contributors"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\DesklandPreview" "InstallLocation" "$INSTDIR"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\DesklandPreview" "DisplayIcon" "$INSTDIR\host\bin\Deskland.exe"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\DesklandPreview" "UninstallString" '$\"$INSTDIR\Uninstall.exe$\"'
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\DesklandPreview" "QuietUninstallString" '$\"$INSTDIR\Uninstall.exe$\" /S'
  WriteRegDWORD HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\DesklandPreview" "NoModify" 1
  WriteRegDWORD HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\DesklandPreview" "NoRepair" 1
  ${GetSize} "$INSTDIR" "/S=0K" $0 $1 $2
  WriteRegDWORD HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\DesklandPreview" "EstimatedSize" $0
  WriteRegStr HKCU "Software\Classes\.deskland-preset" "" "DiyarDesk.Preset"
  WriteRegStr HKCU "Software\Classes\.deskland-preset" "Content Type" "application/json"
  WriteRegStr HKCU "Software\Classes\DiyarDesk.Preset" "" "DiyarDesk Preset"
  WriteRegStr HKCU "Software\Classes\DiyarDesk.Preset\DefaultIcon" "" "$INSTDIR\host\bin\Deskland.exe,0"
  WriteRegStr HKCU "Software\Classes\DiyarDesk.Preset\shell\open\command" "" '$\"$INSTDIR\host\bin\Deskland.exe$\" $\"%1$\"'
  System::Call 'shell32::SHChangeNotify(i 0x08000000, i 0, i 0, i 0)'
SectionEnd

Function un.onInit
  SetShellVarContext current
  SetRegView 64
FunctionEnd
Section "Uninstall"
  IfFileExists "$INSTDIR\host\bin\Deskland.exe" 0 unclosed
  ExecWait '"$INSTDIR\host\bin\Deskland.exe" --exit-installed' $0
  ${If} $0 != 0
    MessageBox MB_ICONSTOP "$(CloseFailed)"
    SetErrorLevel 3
    Abort
  ${EndIf}
  unclosed:
  ReadRegStr $0 HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "DesklandPreview"
  ${UnStrStr} $1 $0 '$\"$INSTDIR\host\bin\Deskland.exe$\"'
  ${If} $1 != ""
    DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "DesklandPreview"
  ${EndIf}
  Delete "$SMSTARTUP\Deskland.lnk"
  !include "${DELETE_LIST}"
  Delete "$INSTDIR\Uninstall.exe"
  RMDir "$INSTDIR"
  Delete "$SMPROGRAMS\DiyarDesk\DiyarDesk.lnk"
  Delete "$SMPROGRAMS\DiyarDesk\Uninstall.lnk"
  RMDir "$SMPROGRAMS\DiyarDesk"
  DeleteRegKey HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\DesklandPreview"
  DeleteRegKey HKCU "Software\DesklandPreview\Installer"
  DeleteRegKey HKCU "Software\Classes\.deskland-preset"
  DeleteRegKey HKCU "Software\Classes\DiyarDesk.Preset"
  System::Call 'shell32::SHChangeNotify(i 0x08000000, i 0, i 0, i 0)'
SectionEnd
