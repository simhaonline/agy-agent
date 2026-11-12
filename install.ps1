# ==============================================================================
# AGY Suite - Single-Line Installer for Windows
# Repository: https://github.com/simhaonline/agy-agent.git
# Usage: irm https://raw.githubusercontent.com/simhaonline/agy-agent/main/install.ps1 | iex
# ==============================================================================

$ErrorActionPreference = "Stop"

$repo = "simhaonline/agy-agent"
$branch = "main"
$rawBase = "https://raw.githubusercontent.com/$repo/$branch"

Write-Host @"
     ___       _______   ____    ____ 
    /   \     /  _____|  \   \  /   / 
   /  ^  \   |  |  __     \   \/   /  
  /  /_\  \  |  | |_ |     \_    _/   
 /  _____  \ |  |__| |       |  |     
/__/     \__\ \______|       |__|     
"@ -ForegroundColor Cyan

Write-Host "Installing AGY Suite (Standalone Rust Agent & Binary Patcher)" -ForegroundColor White
Write-Host "------------------------------------------------------------------" -ForegroundColor Gray

# Check Architecture
if (-not [Environment]::Is64BitOperatingSystem) {
    Write-Host "[ERROR] 32-bit Windows operating system detected." -ForegroundColor Red
    Write-Host "Precompiled Windows binaries are targeted for 64-bit Windows (x86_64)." -ForegroundColor Yellow
    Write-Host "For 32-bit Windows, please build with MSVC 32-bit toolchain: cargo build --release" -ForegroundColor Yellow
    Exit 1
}

# Target Install Directory
$installDir = Join-Path $HOME ".agy\bin"
if (-not (Test-Path $installDir)) {
    New-Item -ItemType Directory -Force -Path $installDir | Out-Null
}

Write-Host "[*] Target installation folder: $installDir" -ForegroundColor Cyan

$binaries = @("agy-agent.exe", "agy-patcher.exe")

foreach ($bin in $binaries) {
    $url = "$rawBase/dist/windows-x64/$bin"
    $dest = Join-Path $installDir $bin
    Write-Host "[*] Downloading $bin..." -ForegroundColor Cyan

    try {
        [Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12 -bor [Net.SecurityProtocolType]::Tls13
        Invoke-WebRequest -Uri $url -OutFile $dest -UseBasicParsing
    } catch {
        Write-Host "[ERROR] Failed to download $bin from $url" -ForegroundColor Red
        Write-Host $_.Exception.Message -ForegroundColor Red
        Exit 1
    }
}

# Update User PATH Environment Variable
$userPath = [Environment]::GetEnvironmentVariable("Path", [EnvironmentVariableTarget]::User)
if ($userPath -split ';' -notcontains $installDir) {
    Write-Host "[*] Adding $installDir to User PATH..." -ForegroundColor Cyan
    $newPath = if ([string]::IsNullOrWhiteSpace($userPath)) { $installDir } else { "$userPath;$installDir" }
    [Environment]::SetEnvironmentVariable("Path", $newPath, [EnvironmentVariableTarget]::User)
    $env:Path = "$installDir;$env:Path"
    Write-Host "[+] PATH environment variable updated permanently." -ForegroundColor Green
} else {
    Write-Host "[*] Installation folder is already in PATH." -ForegroundColor Gray
}

Write-Host "------------------------------------------------------------------" -ForegroundColor Gray
Write-Host "[SUCCESS] AGY Suite installed successfully!" -ForegroundColor Green
Write-Host ""

try {
    & "$installDir\agy-agent.exe" --version
    & "$installDir\agy-patcher.exe" --version
} catch {
    # Non-fatal if execution policy restricts immediate invocation
}

Write-Host ""
Write-Host "Quick Start:" -ForegroundColor Yellow
Write-Host "  agy-agent chat                     # Launch interactive AI assistant"
Write-Host "  agy-agent run `"<objective>`"        # Execute autonomous coding goal"
Write-Host "  agy-agent preset list              # View security and behavior presets"
Write-Host "  agy-patcher inspect <binary>       # Inspect binary PE structure"
Write-Host "------------------------------------------------------------------" -ForegroundColor Gray
