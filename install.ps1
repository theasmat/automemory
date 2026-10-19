# ==============================================================================
# AutoMemory Universal Windows Installer (PowerShell)
# https://github.com/theasmat/automemory-bin
#
# Usage (run in PowerShell):
#   irm https://raw.githubusercontent.com/theasmat/automemory-bin/main/install.ps1 | iex
# ==============================================================================

$ErrorActionPreference = "Stop"

Write-Host ""
Write-Host "========================================================" -ForegroundColor Yellow
Write-Host "       AutoMemory Universal Windows Installer           " -ForegroundColor Yellow
Write-Host "  Persistent Intelligence & Guardrails for AI Agents    "
Write-Host "========================================================" -ForegroundColor Yellow
Write-Host ""

$repo = "theasmat/automemory-bin"
$srcRepo = "theasmat/automemory"
$binaryName = "automemory.exe"

# Target installation directory
$installDir = "$env:USERPROFILE\.cargo\bin"
if (-not (Test-Path $installDir)) {
    New-Item -ItemType Directory -Path $installDir -Force | Out-Null
}

# Detect CPU Architecture
$arch = $env:PROCESSOR_ARCHITECTURE
switch ($arch) {
    "AMD64" { $targetTriple = "x86_64-pc-windows-msvc" }
    "ARM64" { $targetTriple = "aarch64-pc-windows-msvc" }
    default {
        Write-Warning "Unsupported Windows architecture: $arch. Defaulting to x86_64."
        $targetTriple = "x86_64-pc-windows-msvc"
    }
}

Write-Host "==> Detected Platform: Windows ($arch) -> Target: $targetTriple" -ForegroundColor Cyan

$installed = $false

# 1. Check if running inside cloned repo
$scriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path -ErrorAction SilentlyContinue
if ($scriptDir -and (Test-Path "$scriptDir\Cargo.toml")) {
    Write-Host "==> Detected local repository clone at $scriptDir" -ForegroundColor Cyan
    if (Get-Command cargo -ErrorAction SilentlyContinue) {
        Write-Host "==> Compiling release binary with Cargo..." -ForegroundColor Cyan
        Push-Location $scriptDir
        cargo build --release
        Copy-Item "$scriptDir\target\release\$binaryName" "$installDir\$binaryName" -Force
        Pop-Location
        $installed = $true
    }
}

# 2. Try downloading pre-built GitHub release binary
if (-not $installed) {
    $zipUrl = "https://github.com/$repo/releases/latest/download/automemory-$targetTriple.zip"
    $tempZip = [System.IO.Path]::GetTempFileName() + ".zip"
    $tempExtract = Join-Path ([System.IO.Path]::GetTempPath()) "automemory_install_$(Get-Random)"

    Write-Host "==> Attempting to download pre-built release from GitHub ($repo)..." -ForegroundColor Cyan
    try {
        [Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12
        Invoke-WebRequest -Uri $zipUrl -OutFile $tempZip -UseBasicParsing -TimeoutSec 30
        # Verify the download against the release's .sha256 file; refuse to install an unchecked binary.
        try {
            $sumLine = (Invoke-WebRequest -Uri "$zipUrl.sha256" -UseBasicParsing -TimeoutSec 30).Content.ToString().Trim()
        } catch {
            if ($env:AUTOMEMORY_SKIP_CHECKSUM -ne "1") { throw "No .sha256 file next to the release archive; refusing an unchecked download (set AUTOMEMORY_SKIP_CHECKSUM=1 to override)." }
            $sumLine = $null
        }
        if ($sumLine) {
            $want = ($sumLine -split '\s+')[0].ToLower()
            $got = (Get-FileHash -Path $tempZip -Algorithm SHA256).Hash.ToLower()
            if ($want -ne $got) { throw "Checksum mismatch for the downloaded archive; nothing was installed." }
            Write-Host "✓ Checksum verified" -ForegroundColor Green
        }
        Expand-Archive -Path $tempZip -DestinationPath $tempExtract -Force
        if (Test-Path "$tempExtract\$binaryName") {
            Copy-Item "$tempExtract\$binaryName" "$installDir\$binaryName" -Force
            # Unblock binary on Windows to prevent security prompt
            Unblock-File -Path "$installDir\$binaryName" -ErrorAction SilentlyContinue
            $installed = $true
            Write-Host "✓ Downloaded pre-built binary for $targetTriple" -ForegroundColor Green
        }
    } catch {
        Write-Warning "Could not download pre-built release: $($_.Exception.Message)"
    } finally {
        if (Test-Path $tempZip) { Remove-Item -Path $tempZip -Force -ErrorAction SilentlyContinue }
        if (Test-Path $tempExtract) { Remove-Item -Path $tempExtract -Recurse -Force -ErrorAction SilentlyContinue }
    }
}

# 3. Fallback: cargo install
if (-not $installed) {
    if (Get-Command cargo -ErrorAction SilentlyContinue) {
        Write-Host "==> Building from source via cargo install..." -ForegroundColor Cyan
        try {
            cargo install --git "https://github.com/$srcRepo.git" --bin automemory
            $installed = $true
        } catch {
            Write-Warning "cargo install failed: $($_.Exception.Message)"
        }
    }
    
    if (-not $installed) {
        Write-Error "Could not install automemory: Pre-built binary download was unavailable for $targetTriple. Please check https://github.com/$repo/releases"
        exit 1
    }
}

$exePath = Join-Path $installDir $binaryName
Write-Host "✓ Binary installed to: $exePath" -ForegroundColor Green

# 4. Check PATH environment variable
$userPath = [Environment]::GetEnvironmentVariable("Path", [EnvironmentVariableTarget]::User)
if ($userPath -notlike "*$installDir*") {
    Write-Warning "$installDir is not in your User PATH environment variable."
    Write-Host "==> Adding $installDir to User PATH..." -ForegroundColor Cyan
    [Environment]::SetEnvironmentVariable("Path", "$installDir;$userPath", [EnvironmentVariableTarget]::User)
    $env:Path = "$installDir;$env:Path"
}

# 5. Run health diagnostics
Write-Host ""
Write-Host "==> Verifying AutoMemory health diagnostics..." -ForegroundColor Cyan
try {
    & $exePath doctor
} catch {
    Write-Warning "Doctor check encountered a warning: $($_.Exception.Message)"
}

# 6. Offer to connect agents globally
Write-Host ""
Write-Host "==> Configuring universal coding agent connectors on Windows..." -ForegroundColor Cyan
try {
    & $exePath connect all -g
    Write-Host "✓ Configured universal coding agents globally!" -ForegroundColor Green
} catch {
    Write-Warning "Could not connect all agents automatically: $($_.Exception.Message)"
}

Write-Host ""
Write-Host "========================================================" -ForegroundColor Green
Write-Host "  AutoMemory was successfully installed!                " -ForegroundColor Green
try {
    $ver = & $exePath --version
    Write-Host "  Version: $ver" -ForegroundColor Green
} catch {}
Write-Host "========================================================" -ForegroundColor Green
Write-Host ""
Write-Host "Quick commands to get started in PowerShell:"
Write-Host "  automemory serve        Launch local web dashboard at http://127.0.0.1:4545" -ForegroundColor Cyan
Write-Host "  automemory doctor       Run system health checks" -ForegroundColor Cyan
Write-Host "  automemory connect all  Connect all 20 agents in your current workspace" -ForegroundColor Cyan
Write-Host "  automemory proposal summary View review proposal metrics" -ForegroundColor Cyan
Write-Host ""
