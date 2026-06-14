; Multisbox NSIS Installer Script
; Builds a Windows installer for Multisbox multiboxing launcher

!include "MUI2.nsh"
!include "LogicFunc.nsh"

; ============================================
; General Settings
; ============================================
Name "Multisbox"
OutFile "Multisbox-Setup.exe"
InstallDir "$PROGRAMFILES\Multisbox"
InstallDirRegKey HKLM "Software\Multisbox" "InstallDir"
RequestExecutionLevel admin
Unicode True

; Version info (update these for each release)
!define VERSION "0.1.0"
!define PUBLISHER "Coding-Dev-Tools"
!define DESCRIPTION "Multiboxing launcher and window manager"
!define URL "https://github.com/Coding-Dev-Tools/gw2-multibox"

; ============================================
; Interface Settings
; ============================================
!define MUI_ABORTWARNING
!define MUI_ICON "installer\multisbox.ico"
!define MUI_UNICON "installer\multisbox.ico"
!define MUI_HEADERIMAGE
!define MUI_HEADERIMAGE_BITMAP "installer\header.bmp"
!define MUI_WELCOMEFINISHPAGE_BITMAP "installer\welcome.bmp"
!define MUI_UNWELCOMEFINISHPAGE_BITMAP "installer\welcome.bmp"

; ============================================
; Pages
; ============================================
; Installer pages
!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_LICENSE "LICENSE"
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_PAGE_FINISH

; Uninstaller pages
!insertmacro MUI_UNPAGE_WELCOME
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES
!insertmacro MUI_UNPAGE_FINISH

; ============================================
; Languages
; ============================================
!insertmacro MUI_LANGUAGE "English"

; ============================================
; Installer Sections
; ============================================
Section "Multisbox (required)" SecMain
    SectionIn RO

    ; Set output path to the installation directory
    SetOutPath "$INSTDIR"

    ; Install files
    File "target\release\gw2-multibox.exe"
    File "LICENSE"
    File "README.md"
    File "config.yaml.example"

    ; Copy user guide if it exists
    IfFileExists "docs\user-guide.md" 0 +2
        File "docs\user-guide.md"

    ; Store installation path
    WriteRegStr HKLM "Software\Multisbox" "InstallDir" "$INSTDIR"

    ; Create uninstaller
    WriteUninstaller "$INSTDIR\Uninstall.exe"

    ; Add to Add/Remove Programs
    WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\Multisbox" \
        "DisplayName" "Multisbox"
    WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\Multisbox" \
        "UninstallString" '"$INSTDIR\Uninstall.exe"'
    WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\Multisbox" \
        "InstallLocation" "$INSTDIR"
    WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\Multisbox" \
        "DisplayVersion" "${VERSION}"
    WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\Multisbox" \
        "Publisher" "${PUBLISHER}"
    WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\Multisbox" \
        "URLInfoAbout" "${URL}"
    WriteRegDWORD HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\Multisbox" \
        "NoModify" 1
    WriteRegDWORD HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\Multisbox" \
        "NoRepair" 1

    ; Create Start Menu shortcuts
    CreateDirectory "$SMPROGRAMS\Multisbox"
    CreateShortCut "$SMPROGRAMS\Multisbox\Multisbox.lnk" "$INSTDIR\gw2-multibox.exe"
    CreateShortCut "$SMPROGRAMS\Multisbox\Config Editor.lnk" "$INSTDIR\gw2-multibox.exe" "--ui"
    CreateShortCut "$SMPROGRAMS\Multisbox\Uninstall.lnk" "$INSTDIR\Uninstall.exe"

    ; Create desktop shortcut (optional)
    CreateShortCut "$DESKTOP\Multisbox.lnk" "$INSTDIR\gw2-multibox.exe"
SectionEnd

Section "GW2 Quick Setup" SecGW2
    ; Run gw2-init to create default config
    DetailPrint "Creating GW2 configuration..."
    ExecWait '"$INSTDIR\gw2-multibox.exe" gw2-init -c "$INSTDIR\config.yaml"'
SectionEnd

; ============================================
; Uninstaller Section
; ============================================
Section "Uninstall"
    ; Remove files
    Delete "$INSTDIR\gw2-multibox.exe"
    Delete "$INSTDIR\LICENSE"
    Delete "$INSTDIR\README.md"
    Delete "$INSTDIR\config.yaml.example"
    Delete "$INSTDIR\user-guide.md"
    Delete "$INSTDIR\config.yaml"
    Delete "$INSTDIR\Uninstall.exe"

    ; Remove directories
    RMDir "$INSTDIR"

    ; Remove Start Menu shortcuts
    Delete "$SMPROGRAMS\Multisbox\Multisbox.lnk"
    Delete "$SMPROGRAMS\Multisbox\Config Editor.lnk"
    Delete "$SMPROGRAMS\Multisbox\Uninstall.lnk"
    RMDir "$SMPROGRAMS\Multisbox"

    ; Remove desktop shortcut
    Delete "$DESKTOP\Multisbox.lnk"

    ; Remove registry keys
    DeleteRegKey HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\Multisbox"
    DeleteRegKey HKLM "Software\Multisbox"
SectionEnd

; ============================================
; Callbacks
; ============================================
Function .onInit
    ; Check if already installed
    ReadRegStr $0 HKLM "Software\Multisbox" "InstallDir"
    StrCmp $0 "" done

    MessageBox MB_YESNO|MB_ICONQUESTION \
        "Multisbox is already installed. Do you want to reinstall?" \
        IDYES done IDNO abort

abort:
    Abort

done:
FunctionEnd

Function .onInstSuccess
    MessageBox MB_OK \
        "Multisbox has been installed successfully!$\n$\n\
        You can find Multisbox in your Start Menu or on your Desktop.$\n$\n\
        Click Finish to close this installer."
FunctionEnd
