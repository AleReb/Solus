$ErrorActionPreference = 'Stop'

$Repo = 'abdullah-binmadhi/Solus'
$Tag = 'v1.0.7-rolling'
$AssetUrl = "https://github.com/$Repo/releases/download/$Tag/solus-windows-x86_64.exe"
$InstallDir = Join-Path $env:LOCALAPPDATA 'solus\bin'
$TargetExe = Join-Path $InstallDir 'solus.exe'

Write-Host "▸ Setting up Solus ($Tag) for Windows..." -ForegroundColor Magenta

# 1. Ensure Scoop & playback dependencies (mpv, ani-cli, yt-dlp, wezterm)
if (-not (Get-Command scoop -ErrorAction SilentlyContinue)) {
    Write-Host "▸ Installing Scoop package manager..." -ForegroundColor Magenta
    Set-ExecutionPolicy -ExecutionPolicy RemoteSigned -Scope CurrentUser -Force
    Invoke-RestMethod -Uri 'https://get.scoop.sh' | Invoke-Expression
}

$Buckets = (scoop bucket list | Out-String)
if ($Buckets -notmatch 'extras') {
    scoop bucket add extras | Out-Null
}

$Needed = @('git', 'mpv', 'ani-cli', 'yt-dlp', 'ffmpeg', 'wezterm')
$ToInstall = @()
foreach ($Pkg in $Needed) {
    if (-not (Get-Command $Pkg -ErrorAction SilentlyContinue)) {
        $ToInstall += $Pkg
    }
}
if ($ToInstall.Count -gt 0) {
    Write-Host "▸ Installing dependencies ($($ToInstall -join ', '))..." -ForegroundColor Magenta
    scoop install @ToInstall
}

# 2. Download prebuilt solus.exe
New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null
Write-Host "▸ Downloading solus-windows-x86_64.exe..." -ForegroundColor Magenta
Invoke-WebRequest -Uri $AssetUrl -OutFile $TargetExe -UseBasicParsing

# 3. Add to User PATH & current session PATH so `solus` works immediately
$UserPath = [Environment]::GetEnvironmentVariable('Path', 'User')
if ($UserPath -notlike "*$InstallDir*") {
    $NewUserPath = if ([string]::IsNullOrWhiteSpace($UserPath)) { $InstallDir } else { "$UserPath;$InstallDir" }
    [Environment]::SetEnvironmentVariable('Path', $NewUserPath, 'User')
}
if ($env:Path -notlike "*$InstallDir*") {
    $env:Path = "$env:Path;$InstallDir"
}

Write-Host "✓ Solus installed to $TargetExe" -ForegroundColor Green
Write-Host "  Run 'solus' in WezTerm (or this PowerShell window) to launch." -ForegroundColor Cyan
