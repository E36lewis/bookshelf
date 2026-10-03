# Builds Bookshelf's Windows installer:
#   windows/build/installer/Bookshelf-<version>-setup-x64.exe          (-Channel Stable, the release)
#   windows/build/installer/Bookshelf-Preview-<version>-setup-x64.exe  (-Channel Preview, the default)
#
#   windows/installer/build-installer.ps1 [-Channel Stable|Preview] [-Iscc <path to ISCC.exe>]
#
# Needs the Rust core built first (cargo build --release -p bookshelf-ffi,
# with RUSTFLAGS=-C target-feature=+crt-static as windows.yml does), the
# .NET 10 SDK, Python 3 and Inno Setup 7. Publishes a folder build of the
# app for the channel (no single exe: nothing to unpack at startup),
# checks it with scripts/check-windows-build.py and packs it with
# Bookshelf.iss. The version is Bookshelf.csproj's.

param(
    [ValidateSet('Stable', 'Preview')] [string] $Channel = 'Preview',
    [string] $Iscc = 'ISCC.exe'
)

$ErrorActionPreference = 'Stop'
$root = (Resolve-Path (Join-Path $PSScriptRoot '../..')).Path
$project = Join-Path $root 'windows/Bookshelf/Bookshelf.csproj'
$version = ([xml](Get-Content $project)).Project.PropertyGroup.Version | Where-Object { $_ } | Select-Object -First 1
if ($version -notmatch '^\d+\.\d+\.\d+$') { throw "Bookshelf.csproj has no x.y.z <Version>" }
if (-not (Test-Path (Join-Path $root 'target/release/bookshelf_ffi.dll'))) {
    throw 'Build the Rust core first: cargo build --release -p bookshelf-ffi'
}

# Full Windows paths (ISCC is given them).
$app = [IO.Path]::GetFullPath((Join-Path $root "windows\build\installer-app\$Channel"))
$out = [IO.Path]::GetFullPath((Join-Path $root "windows\build\installer"))
if (Test-Path $app) { Remove-Item -Recurse -Force $app }

Write-Host "==> Publishing Bookshelf $version ($Channel) to $app"
dotnet publish $project -c Release -r win-x64 -p:Platform=x64 "-p:BookshelfChannel=$Channel" `
    -p:PublishSingleFile=false -p:IncludeAllContentForSelfExtract=false -o $app
if ($LASTEXITCODE) { throw 'dotnet publish failed' }

Write-Host '==> Checking it starts on every Windows 10/11 PC (CET marking, icon, DLLs)'
python (Join-Path $root 'scripts/check-windows-build.py') $app
if ($LASTEXITCODE) { throw 'check-windows-build.py failed' }

Write-Host "==> Packing it with Inno Setup"
& $Iscc /Q "/DChannel=$Channel" "/DAppVersion=$version" "/DPublishDir=$app" "/O$out" (Join-Path $PSScriptRoot 'Bookshelf.iss')
if ($LASTEXITCODE) { throw 'Inno Setup (ISCC) failed' }
Get-ChildItem $out -Filter '*-setup-x64.exe' | ForEach-Object { Write-Host ("Built {0} ({1:N0} MB)" -f $_.FullName, ($_.Length / 1MB)) }
