; Inno Setup script for the IEEE 1815.2 Test Tool Windows installer.
; Built by .github/workflows/windows-installer.yml, which passes
; AppVersion and VersionInfoVersion via /D. The defaults below only
; cover a manual local iscc run.
#ifndef AppVersion
  #define AppVersion "0.0.0"
#endif
#ifndef VersionInfoVersion
  #define VersionInfoVersion "0.0.0.0"
#endif

[Setup]
; Fixed once: changing this GUID breaks in-place upgrades for everyone
; who already installed under the old AppId.
AppId={{e6851846-fa2b-4b89-93a6-cb0d1dd6d0e6}
AppName=IEEE 1815.2 Test Tool
AppVersion={#AppVersion}
VersionInfoVersion={#VersionInfoVersion}
AppPublisher=Battelle Memorial Institute
DefaultDirName={autopf}\IEEE 1815.2 Test Tool
DefaultGroupName=IEEE 1815.2 Test Tool
DisableProgramGroupPage=yes
; Per-user by default; the admin override is available at install time
; from the command line or the elevation dialog.
PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=dialog commandline
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
MinVersion=10.0
OutputDir=..\target\installer
OutputBaseFilename=ieee-1815-2-test-tool-setup
Compression=lzma2
SolidCompression=yes
; Restart Manager closes a running web_server.exe on upgrade; it is not
; auto-restarted, since the console launcher is the restart mechanism.
CloseApplications=yes
RestartApplications=no
LicenseFile=..\LICENSE

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"

[Files]
; The three release binaries are a build output of the workflow's cargo
; step; they do not exist in the source tree.
Source: "..\target\release\web_server.exe"; DestDir: "{app}\bin"; Flags: ignoreversion
Source: "..\target\release\reference-outstation.exe"; DestDir: "{app}\bin"; Flags: ignoreversion
Source: "..\target\release\reference-control-station.exe"; DestDir: "{app}\bin"; Flags: ignoreversion
; frontend\dist is a build output of the workflow's npm run build step.
Source: "..\frontend\dist\*"; DestDir: "{app}\frontend"; Flags: ignoreversion recursesubdirs createallsubdirs
; Everything in data/ except working/ (user profiles, never shipped) and
; README.md (repo-only contributor doc), so a new top-level data file
; ships without an edit here.
Source: "..\data\*"; DestDir: "{app}\data"; Excludes: "working\*,README.md"; Flags: ignoreversion recursesubdirs createallsubdirs
Source: "launch.cmd"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\LICENSE"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\Disclaimer.txt"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{group}\IEEE 1815.2 Test Tool"; Filename: "{app}\launch.cmd"; WorkingDir: "{app}"
Name: "{group}\Uninstall IEEE 1815.2 Test Tool"; Filename: "{uninstallexe}"

[Run]
; skipifsilent keeps a silent (CI or scripted) install from auto-launching.
; not IsAdmin: postinstall runs with the token Setup started with.
; IsAdminInstallMode alone misses a per-user Setup launched from an
; already-elevated prompt, which would leave the unauthenticated loopback
; server running elevated. The checkbox is then unavailable under any
; elevated token; that is intended.
Filename: "{app}\launch.cmd"; Description: "Launch IEEE 1815.2 Test Tool now"; Flags: postinstall skipifsilent nowait; Check: not IsAdmin
