; Installer for a Zant UI app (Inno Setup 6.3+), compiled by windows.ps1
; for `zant build` with ISCC /D defines: AppTitle, AppVersion, FileVersion,
; AppId, Publisher, SourceDir, OutputDir, OutputName, and optionally
; IconFile, Chinese (path of the Simplified Chinese messages), Sign.
; Per-user install by default (no administrator rights), into
; %LOCALAPPDATA%\Programs\<title>, without asking.
#define AppExe AppTitle + ".exe"

[Setup]
AppId={#AppId}
AppName={#AppTitle}
AppVersion={#AppVersion}
AppVerName={#AppTitle} {#AppVersion}
AppPublisher={#Publisher}
VersionInfoVersion={#FileVersion}
VersionInfoProductName={#AppTitle}
VersionInfoCompany={#Publisher}
DefaultDirName={autopf}\{#AppTitle}
DisableProgramGroupPage=yes
PrivilegesRequired=lowest
; /ALLUSERS on the command line installs for all users instead
PrivilegesRequiredOverridesAllowed=commandline
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
MinVersion=10.0
OutputDir={#OutputDir}
OutputBaseFilename={#OutputName}
#ifdef IconFile
SetupIconFile={#IconFile}
#endif
UninstallDisplayIcon={app}\{#AppExe}
UninstallDisplayName={#AppTitle}
Compression=lzma2/max
SolidCompression=yes
WizardStyle=modern
; the language follows Windows; the choice is shown only when none matches
ShowLanguageDialog=auto
; a running copy is ended by [Code] below before its files are replaced:
; asking it to close (Restart Manager) would act like the close button, and
; an app may answer that with a question (ClipHop: "minimize to tray?")
CloseApplications=no
RestartApplications=no
#ifdef Sign
SignTool=zant
SignedUninstaller=yes
#endif

[Languages]
Name: "en"; MessagesFile: "compiler:Default.isl"
#ifdef Chinese
Name: "zh"; MessagesFile: "{#Chinese}"
#endif

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked

[Files]
Source: "{#SourceDir}\*"; DestDir: "{app}"; Flags: ignoreversion recursesubdirs createallsubdirs

[Icons]
Name: "{autoprograms}\{#AppTitle}"; Filename: "{app}\{#AppExe}"
Name: "{autodesktop}\{#AppTitle}"; Filename: "{app}\{#AppExe}"; Tasks: desktopicon

[Run]
Filename: "{app}\{#AppExe}"; Description: "{cm:LaunchProgram,{#StringChange(AppTitle, '&', '&&')}}"; Flags: nowait postinstall skipifsilent

[Code]
procedure EndRunningApp();
var
  Code: Integer;
begin
  Exec(ExpandConstant('{sys}\taskkill.exe'), '/F /IM "{#AppExe}"', '', SW_HIDE, ewWaitUntilTerminated, Code);
end;

function PrepareToInstall(var NeedsRestart: Boolean): String;
begin
  EndRunningApp();
  Result := '';
end;

function InitializeUninstall(): Boolean;
begin
  EndRunningApp();
  Result := True;
end;

// zant.desktop.setLaunchAtLogin stores a Run value named after the app id;
// remove it with the app so Windows does not try to start a missing exe
procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
begin
  if CurUninstallStep = usPostUninstall then
    RegDeleteValue(HKEY_CURRENT_USER, 'Software\Microsoft\Windows\CurrentVersion\Run', '{#AppId}');
end;
