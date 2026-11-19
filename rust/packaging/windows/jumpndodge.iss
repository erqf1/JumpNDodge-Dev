; JumpNDodge Rust Edition - Windows-Installer (Inno Setup 6)
; Bauen: iscc /DMyAppVersion=5.0.0 rust\packaging\windows\jumpndodge.iss
; Erwartet das portable Paket in rust\dist\JumpNDodge (siehe .github/workflows/build-rust.yml).
;
; Gleiche AppId wie der bisherige JumpNDodge-Installer (Python-Version): das Setup aktualisiert eine
; bestehende Installation an Ort und Stelle. Der Ordner data\ (Spielstand, Sounds) bleibt unangetastet,
; Reste der Python-Version (pygame, Python-Laufzeit, separater Updater) werden entfernt.
#ifndef MyAppVersion
  #define MyAppVersion "5.0.0"
#endif

[Setup]
AppId={{B44A592F-4DB5-4419-BCA6-7C5F80AA4472}
AppName=JumpNDodge
AppVersion={#MyAppVersion}
AppVerName=JumpNDodge {#MyAppVersion}
AppPublisher=W Software
AppPublisherURL=https://mert.win/jump/
AppSupportURL=https://mert.win/jump/
AppUpdatesURL=https://github.com/erqf1/JumpNDodge/releases
DefaultDirName={autopf}\JumpNDodge
DefaultGroupName=JumpNDodge
UninstallDisplayIcon={app}\JumpNDodge.exe
UninstallDisplayName=JumpNDodge
SetupIconFile=..\..\..\icon.ico
OutputDir=..\..\dist
OutputBaseFilename=JumpNDodge-windows-x64-setup
Compression=lzma2/max
SolidCompression=yes
WizardStyle=modern
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=dialog
DisableProgramGroupPage=yes
DisableDirPage=no
UsePreviousAppDir=yes
CloseApplications=yes
VersionInfoVersion={#MyAppVersion}
VersionInfoProductName=JumpNDodge

[Languages]
Name: "german"; MessagesFile: "compiler:Languages\German.isl"
Name: "english"; MessagesFile: "compiler:Default.isl"

[Tasks]
; Nur bei der Erst-Installation angeboten. Bei Updates (auch still über den eingebauten Updater, der sonst die
; gemerkte Auswahl wiederholt) wird der Desktop nie angefasst - ein gelöschtes Symbol bleibt gelöscht.
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked; Check: IsFirstInstall

[InstallDelete]
; Reste der Python-Version - data\ wird nie angefasst
Type: files; Name: "{app}\updater.exe"
Type: files; Name: "{app}\version.txt"
Type: files; Name: "{app}\updater.log"
Type: filesandordirs; Name: "{app}\pygame"
Type: files; Name: "{app}\*.pyd"
Type: files; Name: "{app}\python3*.dll"
Type: files; Name: "{app}\libcrypto-3.dll"
Type: files; Name: "{app}\libssl-3.dll"
Type: files; Name: "{app}\libffi-8.dll"
Type: files; Name: "{app}\vcruntime140*.dll"
Type: files; Name: "{autodesktop}\JumpNDodge Updater.lnk"

[Files]
Source: "..\..\dist\JumpNDodge\*"; DestDir: "{app}"; Flags: recursesubdirs ignoreversion

[Dirs]
; Spielstand neben dem Programm (wie bisher) - beschreibbar auch bei Installation für alle Benutzer
Name: "{app}\data"; Permissions: users-modify

[Icons]
Name: "{autoprograms}\JumpNDodge"; Filename: "{app}\JumpNDodge.exe"
; Desktop-Verknüpfung nur bei der Erst-Installation (und nur, wenn es noch keine gibt). Bei Updates bleibt der
; Desktop unangetastet: vorhandene Symbole springen nicht ans Ende, gelöschte kommen nicht wieder.
Name: "{autodesktop}\JumpNDodge"; Filename: "{app}\JumpNDodge.exe"; Tasks: desktopicon; Check: IsFirstInstall and NoDesktopIconYet

[Run]
Filename: "{app}\JumpNDodge.exe"; Description: "{cm:LaunchProgram,JumpNDodge}"; Flags: nowait postinstall skipifsilent

[Code]
const
  UninstKey = 'Software\Microsoft\Windows\CurrentVersion\Uninstall\{B44A592F-4DB5-4419-BCA6-7C5F80AA4472}_is1';

var
  HadPreviousInstall: Boolean;  // Uninstall-Eintrag gefunden (Rust- oder alte Python-Version, gleiche AppId)
  ExeCheckDone: Boolean;
  ExeExisted: Boolean;          // JumpNDodge.exe lag vor dem Kopieren schon im Zielordner

function InitializeSetup: Boolean;
begin
  // alle Benutzer (64 und 32 Bit, der alte Installer war 32 Bit) und nur ich
  HadPreviousInstall := RegKeyExists(HKLM64, UninstKey) or RegKeyExists(HKLM32, UninstKey)
                     or RegKeyExists(HKCU, UninstKey);
  ExeCheckDone := False;
  ExeExisted := False;
  Result := True;
end;

procedure CurStepChanged(CurStep: TSetupStep);
begin
  // vor dem Kopieren merken, ob das Spiel schon da war (z.B. Installation ohne Uninstall-Eintrag)
  if CurStep = ssInstall then
  begin
    ExeExisted := FileExists(ExpandConstant('{app}\JumpNDodge.exe'));
    ExeCheckDone := True;
  end;
end;

// Erst-Installation: kein Uninstall-Eintrag und noch kein Spiel im Zielordner
function IsFirstInstall: Boolean;
begin
  Result := not HadPreviousInstall;
  if Result then
  begin
    if ExeCheckDone then
      Result := not ExeExisted
    else
      Result := not FileExists(AddBackslash(WizardDirValue) + 'JumpNDodge.exe');
  end;
end;

function NoDesktopIconYet: Boolean;
begin
  // beide Desktops prüfen (alle Benutzer / nur ich), egal in welchem Modus installiert wird
  Result := not FileExists(ExpandConstant('{autodesktop}\JumpNDodge.lnk'))
        and not FileExists(ExpandConstant('{userdesktop}\JumpNDodge.lnk'))
        and not FileExists(ExpandConstant('{commondesktop}\JumpNDodge.lnk'));
end;
