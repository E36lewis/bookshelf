; Bookshelf's Windows installer, for Inno Setup 7. build-installer.ps1
; publishes the app and compiles this with:
;
;   ISCC /DChannel=Stable|Preview /DAppVersion=0.9.0 /DPublishDir=<published app> Bookshelf.iss
;
; Per user, no admin: Bookshelf goes in %LOCALAPPDATA%\Programs\Bookshelf,
; with a Start menu entry and an entry in Settings > Apps. Uninstalling
; removes the program only: the journal (%LOCALAPPDATA%\Bookshelf) is
; never touched, as it isn't one of the files installed here.
;
; Stable is the release. Preview installs "Bookshelf Preview", which has
; its own folder, journal and Settings entry (see ChannelNames.cs).

#ifndef AppVersion
  #error Pass the version: /DAppVersion=x.y.z
#endif
#ifndef PublishDir
  #error Pass the published app's folder: /DPublishDir=...
#endif
#ifndef Channel
  #define Channel "Preview"
#endif
#if Channel == "Stable"
  #define AppName "Bookshelf"
  #define AppId "io.github.e36lewis.Bookshelf"
  #define OutputName "Bookshelf-" + AppVersion + "-setup-x64"
#elif Channel == "Preview"
  #define AppName "Bookshelf Preview"
  #define AppId "io.github.e36lewis.Bookshelf.Preview"
  #define OutputName "Bookshelf-Preview-" + AppVersion + "-setup-x64"
#else
  #error Channel must be Stable or Preview
#endif

[Setup]
AppId={#AppId}
AppName={#AppName}
AppVersion={#AppVersion}
AppVerName={#AppName} {#AppVersion}
AppPublisher=e36lewis
AppPublisherURL=https://github.com/E36lewis/bookshelf
AppSupportURL=https://github.com/E36lewis/bookshelf/issues
AppCopyright=Copyright © 2026 e36lewis. Free software under the GPL 3.0 or later.
VersionInfoVersion={#AppVersion}
VersionInfoProductName={#AppName}
; 64-bit Setup, for 64-bit Windows on x64 processors only (x64 is the only
; build for now). Being 64-bit also lets CetSupportMissing ask Windows
; exactly what .NET will.
SetupArchitecture=x64
ArchitecturesAllowed=x64os
ArchitecturesInstallIn64BitMode=x64os
; Windows 10 version 1809, the Windows App SDK's minimum.
MinVersion=10.0.17763
; Just for this user: no admin, no UAC prompt.
PrivilegesRequired=lowest
DefaultDirName={autopf}\{#AppName}
DisableDirPage=yes
DisableProgramGroupPage=yes
WizardStyle=modern
SetupIconFile=..\Bookshelf\Assets\Bookshelf.ico
UninstallDisplayIcon={app}\Bookshelf.exe
UninstallDisplayName={#AppName}
; The app holds this mutex while it's open (Program.cs), so Setup and the
; uninstaller ask for it to be closed first. Restart Manager also offers to
; close it when an update replaces its files.
AppMutex={#AppId}
CloseApplications=yes
RestartApplications=no
OutputBaseFilename={#OutputName}
Compression=lzma2/max
SolidCompression=yes

[Tasks]
Name: desktopicon; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked

[Files]
Source: "{#PublishDir}\*"; DestDir: "{app}"; Excludes: "*.pdb"; Flags: ignoreversion recursesubdirs createallsubdirs

[Icons]
Name: "{autoprograms}\{#AppName}"; Filename: "{app}\Bookshelf.exe"
Name: "{autodesktop}\{#AppName}"; Filename: "{app}\Bookshelf.exe"; Tasks: desktopicon

[Run]
Filename: "{app}\Bookshelf.exe"; Description: "{cm:LaunchProgram,{#AppName}}"; Flags: nowait postinstall skipifsilent

[Messages]
ConfirmUninstall=Remove %1 from this PC?%n%nYour journal stays: your books and summaries are kept in AppData\Local\{#AppName} in your user folder, and %1 finds them again if you install it later.

[CustomMessages]
CetSupportMissing={#AppName} needs a newer Windows update.%n%nOpen Settings › Windows Update, install the updates (restart if it asks), then run this setup again.

[Code]
// .NET, which Bookshelf runs on, stops at startup with "Your Windows
// doesn't fully support CET" (0x80131506) on a PC whose processor has CET
// (Intel 11th generation and AMD Ryzen 5000 on, roughly) when Windows is
// missing the update that lets it interrupt a thread with a "special
// user-mode APC" (QueueUserAPC2). Older processors never need it.
//
// This asks Windows the same two questions .NET asks
// (Thread::StaticInitialize in dotnet/runtime src/coreclr/vm/threads.cpp,
// release/10.0): are shadow stacks on for apps, and does QueueUserAPC2
// take a special APC with its CONTEXT? Only then does .NET stop.
//
// Which updates have it isn't documented. From dotnet/runtime issues
// 108589, 110000, 110920 and 121922 (2024-2025): Windows 10 22H2 and 21H2
// stopped with updates up to July 2024 (19045.4651) and ran from the
// October 2024 update on (KB5044273, ntdll 10.0.19041.5007); Windows 11
// 21H2 stopped at 22000.1696 (March 2023); Windows 11 22H2 and later have
// it. Breaking change: https://learn.microsoft.com/dotnet/core/compatibility/interop/9.0/cet-support
// A fully updated Windows 10 22H2 or Windows 11 always has it.

const
  USER_CET_ENVIRONMENT_WIN32_PROCESS = 0;
  QUEUE_USER_APC_FLAGS_SPECIAL_USER_APC = $1;
  QUEUE_USER_APC_CALLBACK_DATA_CONTEXT = $10000;

// Both are missing from older Windows: delayload makes a call to a missing
// one an exception rather than stopping the whole script.
function IsUserCetAvailableInEnvironment(UserCetEnvironment: DWORD): BOOL;
  external 'IsUserCetAvailableInEnvironment@kernel32.dll stdcall delayload';
function QueueUserAPC2(ApcRoutine: NativeUInt; Thread: NativeUInt; Data: NativeUInt; Flags: DWORD): BOOL;
  external 'QueueUserAPC2@kernel32.dll stdcall delayload';
function GetCurrentThread(): NativeUInt;
  external 'GetCurrentThread@kernel32.dll stdcall';
function GetModuleHandle(ModuleName: String): NativeUInt;
  external 'GetModuleHandleW@kernel32.dll stdcall';
function GetProcAddress(Module: NativeUInt; ProcName: AnsiString): NativeUInt;
  external 'GetProcAddress@kernel32.dll stdcall';

// True if .NET would stop at startup on this PC.
function CetSupportMissing(): Boolean;
var
  ShadowStacks, SpecialApcs: Boolean;
  NoOp: NativeUInt;
begin
  ShadowStacks := False;
  try
    if IsUserCetAvailableInEnvironment(USER_CET_ENVIRONMENT_WIN32_PROCESS) then
      ShadowStacks := True;
  except
    Log('CET: IsUserCetAvailableInEnvironment is missing (Windows before 10 version 2004)');
  end;
  if not ShadowStacks then
  begin
    Log('CET: no shadow stacks for apps on this PC, so nothing to check');
    Result := False;
    exit;
  end;

  // .NET queues an empty APC to its own thread. GetTickCount takes no
  // arguments and changes nothing, so it does as an empty callback (an x64
  // callee ignores arguments it doesn't take).
  SpecialApcs := False;
  NoOp := GetProcAddress(GetModuleHandle('kernel32.dll'), 'GetTickCount');
  if NoOp <> 0 then
  try
    if QueueUserAPC2(NoOp, GetCurrentThread(), 0,
        QUEUE_USER_APC_FLAGS_SPECIAL_USER_APC or QUEUE_USER_APC_CALLBACK_DATA_CONTEXT) then
      SpecialApcs := True;
  except
    Log('CET: QueueUserAPC2 is missing');
  end;
  if SpecialApcs then
    Log('CET: shadow stacks are on and Windows has the update .NET needs')
  else
    Log('CET: shadow stacks are on but Windows lacks the update .NET needs');
  Result := not SpecialApcs;
end;

function InitializeSetup(): Boolean;
begin
  Result := True;
  if CetSupportMissing() then
  begin
    SuppressibleMsgBox(CustomMessage('CetSupportMissing'), mbCriticalError, MB_OK, IDOK);
    Result := False;
  end;
end;
