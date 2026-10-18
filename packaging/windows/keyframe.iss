#ifndef AppVersion
  #define AppVersion "0.1.0"
#endif
#ifndef RuntimeDir
  #error Use scripts/package-windows.ps1 to include the Visual C++ runtime.
#endif
#ifndef BinaryPath
  #define BinaryPath "..\..\target\release\keyframe.exe"
#endif

[Setup]
AppId={{7762F644-EE4F-460B-9E2C-5A7106B633A8}
AppName=Keyframe
AppVersion={#AppVersion}
AppPublisher=Keyframe
DefaultDirName={localappdata}\Programs\Keyframe
DefaultGroupName=Keyframe
PrivilegesRequired=lowest
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
MinVersion=10.0
OutputDir=..\..\dist
OutputBaseFilename=keyframe-windows-x64-setup
Compression=lzma2
SolidCompression=yes
UninstallDisplayIcon={app}\keyframe.exe
SetupIconFile=keyframe.ico
VersionInfoVersion={#AppVersion}
DisableProgramGroupPage=yes
DisableWelcomePage=no
CloseApplications=yes
CloseApplicationsFilter=keyframe.exe
RestartApplications=no
ChangesAssociations=yes
WizardStyle=modern

[Tasks]
Name: "desktopicon"; Description: "Create a desktop shortcut"

[Files]
Source: "{#BinaryPath}"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#RuntimeDir}\*.dll"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{autoprograms}\Keyframe"; Filename: "{app}\keyframe.exe"; WorkingDir: "{app}"; Comment: "Present PDF slides"; AppUserModelID: "dev.keyframe.keyframe"
Name: "{autodesktop}\Keyframe"; Filename: "{app}\keyframe.exe"; WorkingDir: "{app}"; Comment: "Present PDF slides"; AppUserModelID: "dev.keyframe.keyframe"; Tasks: desktopicon

[Registry]
Root: HKCU; Subkey: "Software\Classes\Applications\keyframe.exe"; ValueType: string; ValueName: "FriendlyAppName"; ValueData: "Keyframe"; Flags: uninsdeletekey
Root: HKCU; Subkey: "Software\Classes\Applications\keyframe.exe\DefaultIcon"; ValueType: string; ValueData: "{app}\keyframe.exe,0"
Root: HKCU; Subkey: "Software\Classes\Applications\keyframe.exe\SupportedTypes"; ValueType: string; ValueName: ".pdf"; ValueData: ""
Root: HKCU; Subkey: "Software\Classes\Applications\keyframe.exe\shell\open\command"; ValueType: string; ValueData: """{app}\keyframe.exe"" ""%1"""
Root: HKCU; Subkey: "Software\Classes\Keyframe.PDF"; ValueType: string; ValueData: "PDF presentation"; Flags: uninsdeletekey
Root: HKCU; Subkey: "Software\Classes\Keyframe.PDF\DefaultIcon"; ValueType: string; ValueData: "{app}\keyframe.exe,0"
Root: HKCU; Subkey: "Software\Classes\Keyframe.PDF\shell\open\command"; ValueType: string; ValueData: """{app}\keyframe.exe"" ""%1"""
Root: HKCU; Subkey: "Software\Classes\.pdf\OpenWithProgids"; ValueType: none; ValueName: "Keyframe.PDF"; Flags: uninsdeletevalue

[Run]
Filename: "{app}\keyframe.exe"; Description: "Launch Keyframe"; Flags: nowait postinstall skipifsilent
