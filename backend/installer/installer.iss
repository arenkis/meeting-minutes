; Inno Setup Script for Meeting Minutes Backend
; Creates a complete Windows installer for non-technical users

#define MyAppName "Meeting Minutes Backend"
#define MyAppVersion "1.0.0"
#define MyAppPublisher "Meeting Minutes Team"
#define MyAppURL "https://github.com/zackriya-solutions/meeting-minutes"
#define MyAppExeName "meeting-minutes-launcher.cmd"

[Setup]
AppId={{A7B3C4D5-E6F7-8901-2345-6789ABCDEF01}
AppName={#MyAppName}
AppVersion={#MyAppVersion}
AppPublisher={#MyAppPublisher}
AppPublisherURL={#MyAppURL}
AppSupportURL={#MyAppURL}
AppUpdatesURL={#MyAppURL}
DefaultDirName={autopf}\MeetingMinutes
DefaultGroupName={#MyAppName}
AllowNoIcons=yes
LicenseFile=..\LICENSE.txt
InfoBeforeFile=..\README.md
OutputDir=output
OutputBaseFilename=MeetingMinutesSetup-{#MyAppVersion}
SetupIconFile=icon.ico
Compression=lzma2/max
SolidCompression=yes
WizardStyle=modern
DisableProgramGroupPage=yes
PrivilegesRequired=admin
ArchitecturesInstallIn64BitMode=x64
UninstallDisplayIcon={app}\{#MyAppExeName}

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked
Name: "quicklaunchicon"; Description: "{cm:CreateQuickLaunchIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked; OnlyBelowVersion: 6.1; Check: not IsAdminInstallMode
Name: "autostart"; Description: "Start Meeting Minutes Backend on Windows startup"; GroupDescription: "Startup Options:"; Flags: unchecked

[Files]
; Core executables
Source: "meeting-minutes-backend.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\whisper-server-package\whisper-server.exe"; DestDir: "{app}"; Flags: ignoreversion

; Models (user selects during install)
Source: "..\models\ggml-tiny.bin"; DestDir: "{app}\models"; Flags: ignoreversion; Check: IsModelSelected('tiny')
Source: "..\models\ggml-small.bin"; DestDir: "{app}\models"; Flags: ignoreversion; Check: IsModelSelected('small')
Source: "..\models\ggml-medium.bin"; DestDir: "{app}\models"; Flags: ignoreversion; Check: IsModelSelected('medium')
Source: "..\models\ggml-large-v3.bin"; DestDir: "{app}\models"; Flags: ignoreversion; Check: IsModelSelected('large')

; Configuration files
Source: "..\temp.env"; DestDir: "{app}"; DestName: ".env"; Flags: ignoreversion onlyifdoesntexist
Source: "meeting-minutes-launcher.cmd"; DestDir: "{app}"; Flags: ignoreversion

; Public web files
Source: "..\whisper.cpp\examples\server\public\*"; DestDir: "{app}\public"; Flags: ignoreversion recursesubdirs createallsubdirs

; Visual C++ Redistributables
Source: "vcredist_x64.exe"; DestDir: "{tmp}"; Flags: deleteafterinstall

; Documentation
Source: "..\README.md"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\API_DOCUMENTATION.md"; DestDir: "{app}"; Flags: ignoreversion

[Dirs]
Name: "{app}\data"; Permissions: users-full
Name: "{app}\logs"; Permissions: users-full
Name: "{app}\models"; Permissions: users-full

[Icons]
Name: "{group}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"
Name: "{group}\{cm:UninstallProgram,{#MyAppName}}"; Filename: "{uninstallexe}"
Name: "{autodesktop}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"; Tasks: desktopicon
Name: "{userappdata}\Microsoft\Internet Explorer\Quick Launch\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"; Tasks: quicklaunchicon

[Registry]
; Add to Windows startup if selected
Root: HKCU; Subkey: "Software\Microsoft\Windows\CurrentVersion\Run"; ValueType: string; ValueName: "MeetingMinutesBackend"; ValueData: """{app}\{#MyAppExeName}"" --startup"; Flags: uninsdeletevalue; Tasks: autostart

; Store installation settings
Root: HKLM; Subkey: "Software\MeetingMinutes"; ValueType: string; ValueName: "InstallPath"; ValueData: "{app}"; Flags: uninsdeletekey
Root: HKLM; Subkey: "Software\MeetingMinutes"; ValueType: string; ValueName: "Version"; ValueData: "{#MyAppVersion}"
Root: HKLM; Subkey: "Software\MeetingMinutes"; ValueType: string; ValueName: "SelectedModel"; ValueData: "{code:GetSelectedModel}"

[Run]
; Install Visual C++ Redistributables silently
Filename: "{tmp}\vcredist_x64.exe"; Parameters: "/quiet /norestart"; StatusMsg: "Installing Visual C++ Redistributables..."; Flags: waituntilterminated

; Initialize database
Filename: "{app}\meeting-minutes-backend.exe"; Parameters: "--init-db"; StatusMsg: "Initializing database..."; Flags: waituntilterminated runhidden

; Optionally start the application
Filename: "{app}\{#MyAppExeName}"; Description: "{cm:LaunchProgram,{#StringChange(MyAppName, '&', '&&')}}"; Flags: nowait postinstall skipifsilent

[UninstallRun]
; Stop services before uninstall
Filename: "{sys}\taskkill.exe"; Parameters: "/F /IM whisper-server.exe"; Flags: runhidden waituntilterminated
Filename: "{sys}\taskkill.exe"; Parameters: "/F /IM meeting-minutes-backend.exe"; Flags: runhidden waituntilterminated

[Code]
var
  ModelPage: TInputOptionWizardPage;
  PortPage: TInputQueryWizardPage;
  SelectedModel: string;
  
procedure InitializeWizard;
begin
  // Create model selection page
  ModelPage := CreateInputOptionPage(wpSelectDir,
    'Select Whisper Model',
    'Choose the AI model size for speech recognition',
    'Larger models are more accurate but require more resources:' + #13#10 +
    '• Tiny (39 MB): Fastest, least accurate' + #13#10 +
    '• Small (244 MB): Good balance for most users' + #13#10 +
    '• Medium (1.5 GB): Better accuracy, slower' + #13#10 +
    '• Large (3.1 GB): Best accuracy, requires powerful PC',
    True, False);
    
  ModelPage.Add('Tiny - Fast, basic accuracy (39 MB)');
  ModelPage.Add('Small - Recommended for most users (244 MB)');
  ModelPage.Add('Medium - Better accuracy (1.5 GB)');
  ModelPage.Add('Large - Best accuracy (3.1 GB)');
  
  // Default to Small model
  ModelPage.SelectedValueIndex := 1;
  
  // Create port configuration page
  PortPage := CreateInputQueryPage(ModelPage.ID,
    'Network Configuration',
    'Configure the server ports',
    'Please specify the ports for the services. Default values should work for most users.');
    
  PortPage.Add('Whisper Server Port:', False);
  PortPage.Add('Backend API Port:', False);
  
  // Set default ports
  PortPage.Values[0] := '8178';
  PortPage.Values[1] := '8000';
end;

function IsModelSelected(Model: String): Boolean;
begin
  case ModelPage.SelectedValueIndex of
    0: Result := Model = 'tiny';
    1: Result := Model = 'small';
    2: Result := Model = 'medium';
    3: Result := Model = 'large';
  else
    Result := False;
  end;
end;

function GetSelectedModel(Param: String): String;
begin
  case ModelPage.SelectedValueIndex of
    0: Result := 'tiny';
    1: Result := 'small';
    2: Result := 'medium';
    3: Result := 'large';
  else
    Result := 'small';
  end;
end;

procedure CurStepChanged(CurStep: TSetupStep);
var
  EnvFile: String;
  EnvContent: TStringList;
begin
  if CurStep = ssPostInstall then
  begin
    // Update .env file with ports
    EnvFile := ExpandConstant('{app}\.env');
    EnvContent := TStringList.Create;
    try
      if FileExists(EnvFile) then
        EnvContent.LoadFromFile(EnvFile);
        
      // Update or add port settings
      EnvContent.Values['WHISPER_PORT'] := PortPage.Values[0];
      EnvContent.Values['BACKEND_PORT'] := PortPage.Values[1];
      EnvContent.Values['WHISPER_MODEL'] := GetSelectedModel('');
      
      EnvContent.SaveToFile(EnvFile);
    finally
      EnvContent.Free;
    end;
  end;
end;

function InitializeUninstall(): Boolean;
begin
  // Stop any running processes
  Result := True;
end;