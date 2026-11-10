; Windows installer for Open Recovr, built by the release workflow with Inno Setup 6:
;   ISCC /DAppVersion=0.3.0 /DSourceDir=<packaged folder> /DIconFile=<.ico> installer\open-recovr.iss
; It installs for the current user only (no administrator rights asked), and the
; in-app update runs it silently: /SP- /VERYSILENT /SUPPRESSMSGBOXES /NORESTART.

#ifndef AppVersion
  #error Pass the version: /DAppVersion=x.y.z
#endif
#ifndef SourceDir
  #define SourceDir "..\dist\open-recovr"
#endif

[Setup]
AppId={{BCECD8AF-19EC-4412-9AA8-C94C765A4CE3}
AppName=Open Recovr
AppVersion={#AppVersion}
AppVerName=Open Recovr {#AppVersion}
AppPublisher=Open Recovr
AppPublisherURL=https://github.com/jlerouge/open-recovr
AppSupportURL=https://github.com/jlerouge/open-recovr/issues
AppUpdatesURL=https://github.com/jlerouge/open-recovr/releases
DefaultDirName={localappdata}\Programs\Open Recovr
DefaultGroupName=Open Recovr
DisableProgramGroupPage=yes
DisableDirPage=auto
PrivilegesRequired=lowest
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
LicenseFile={#SourceDir}\LICENSE
OutputDir=..\dist
OutputBaseFilename=open-recovr-v{#AppVersion}-windows-x64-setup
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
UninstallDisplayIcon={app}\open-recovr.exe
UninstallDisplayName=Open Recovr
; the running program is closed by the update itself; this covers a manual install
CloseApplications=yes
RestartApplications=no
#ifdef IconFile
SetupIconFile={#IconFile}
#endif

[Languages]
Name: "en"; MessagesFile: "compiler:Default.isl"
Name: "fr"; MessagesFile: "compiler:Languages\French.isl"
Name: "es"; MessagesFile: "compiler:Languages\Spanish.isl"
Name: "de"; MessagesFile: "compiler:Languages\German.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked

[Files]
Source: "{#SourceDir}\open-recovr.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#SourceDir}\README.md"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#SourceDir}\CHANGELOG.md"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#SourceDir}\LICENSE"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#SourceDir}\vendor\*"; DestDir: "{app}\vendor"; Flags: ignoreversion recursesubdirs createallsubdirs

[InstallDelete]
; a portable copy's leftover from a past in-place update
Type: files; Name: "{app}\open-recovr.exe.old"

[Icons]
Name: "{group}\Open Recovr"; Filename: "{app}\open-recovr.exe"
Name: "{group}\{cm:UninstallProgram,Open Recovr}"; Filename: "{uninstallexe}"
Name: "{autodesktop}\Open Recovr"; Filename: "{app}\open-recovr.exe"; Tasks: desktopicon

[Run]
; no skipifsilent: after the silent in-app update, this starts the new version
Filename: "{app}\open-recovr.exe"; Description: "{cm:LaunchProgram,Open Recovr}"; Flags: nowait postinstall
