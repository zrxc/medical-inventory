param(
    [string]$Configuration = "release"
)

$ErrorActionPreference = "Stop"

$projectRoot = Split-Path -Parent $PSScriptRoot
$packageName = "medical-inventory-windows-x64"
$distDir = Join-Path $projectRoot "dist"
$packageDir = Join-Path $distDir $packageName
$exeName = "medical_inventory.exe"
$exePath = Join-Path $projectRoot "target\$Configuration\$exeName"
$zipPath = Join-Path $distDir "$packageName.zip"

function Assert-UnderRoot {
    param(
        [string]$Root,
        [string]$PathToCheck
    )

    $rootFullPath = [System.IO.Path]::GetFullPath($Root).TrimEnd('\') + '\'
    $targetFullPath = [System.IO.Path]::GetFullPath($PathToCheck)
    if (-not $targetFullPath.StartsWith($rootFullPath, [System.StringComparison]::OrdinalIgnoreCase)) {
        throw "Refuse to operate outside the project directory: $targetFullPath"
    }
}

Push-Location $projectRoot
try {
    cargo build --release

    if (-not (Test-Path -LiteralPath $exePath)) {
        throw "Release executable was not found: $exePath"
    }

    Assert-UnderRoot -Root $projectRoot -PathToCheck $distDir
    Assert-UnderRoot -Root $distDir -PathToCheck $packageDir
    Assert-UnderRoot -Root $distDir -PathToCheck $zipPath

    if (Test-Path -LiteralPath $packageDir) {
        Remove-Item -LiteralPath $packageDir -Recurse -Force
    }
    New-Item -ItemType Directory -Path $packageDir | Out-Null

    Copy-Item -LiteralPath $exePath -Destination (Join-Path $packageDir "MedicalInventory.exe")
    Copy-Item -LiteralPath (Join-Path $projectRoot "README.md") -Destination $packageDir

    $startScript = @'
@echo off
start "" "%~dp0MedicalInventory.exe"
'@
    Set-Content -Path (Join-Path $packageDir "Start.bat") -Value $startScript -Encoding ASCII

    if (Test-Path -LiteralPath $zipPath) {
        Remove-Item -LiteralPath $zipPath -Force
    }
    Compress-Archive -Path (Join-Path $packageDir "*") -DestinationPath $zipPath -Force

    Write-Host "Portable desktop package created:"
    Write-Host $zipPath
    Write-Host "After unzip, run Start.bat or MedicalInventory.exe"
}
finally {
    Pop-Location
}
