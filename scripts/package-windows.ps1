param(
    [string]$IsccPath,
    [string]$BinaryPath,
    [string]$RuntimeDirectory,
    [switch]$SkipBuild
)

$ErrorActionPreference = 'Stop'
$projectDirectory = Split-Path -Parent $PSScriptRoot
Push-Location $projectDirectory
try {
    if (!$SkipBuild) {
        & cargo build --release --locked
        if ($LASTEXITCODE -ne 0) { throw 'The release build failed.' }
    }
    if (!$BinaryPath) { $BinaryPath = Join-Path $projectDirectory 'target/release/keyframe.exe' }
    $BinaryPath = (Resolve-Path -LiteralPath $BinaryPath).Path
    $version = (Select-String -LiteralPath Cargo.toml -Pattern '^version = "([^"]+)"').Matches[0].Groups[1].Value

    if (!$IsccPath) {
        $compiler = Get-Command ISCC.exe -ErrorAction SilentlyContinue
        $candidates = @(
            $env:ISCC_PATH,
            $compiler.Source,
            "${env:ProgramFiles(x86)}/Inno Setup 6/ISCC.exe",
            "$env:LOCALAPPDATA/Programs/Inno Setup 6/ISCC.exe",
            "$projectDirectory/target/installer-tools/inno/ISCC.exe"
        )
        $IsccPath = $candidates | Where-Object { $_ -and (Test-Path -LiteralPath $_) } | Select-Object -First 1
    }
    if (!$IsccPath) { throw 'Inno Setup 6 was not found. Install it or pass -IsccPath to this script.' }
    $IsccPath = (Resolve-Path -LiteralPath $IsccPath).Path

    # Ship Microsoft's app-local redistributables, not DLLs copied from System32.
    # Windows 10+ supplies the Universal CRT itself.
    if (!$RuntimeDirectory -and $env:VCToolsRedistDir) {
        $candidate = Join-Path $env:VCToolsRedistDir 'x64/Microsoft.VC143.CRT'
        if (Test-Path -LiteralPath $candidate) { $RuntimeDirectory = $candidate }
    }
    if (!$RuntimeDirectory) {
        $vswhere = "${env:ProgramFiles(x86)}/Microsoft Visual Studio/Installer/vswhere.exe"
        if (Test-Path -LiteralPath $vswhere) {
            $visualStudio = & $vswhere -latest -products '*' -property installationPath
            if ($visualStudio) {
                $redistRoot = Join-Path $visualStudio 'VC/Redist/MSVC'
                foreach ($directory in (Get-ChildItem -LiteralPath $redistRoot -Directory | Sort-Object Name -Descending)) {
                    $candidate = Join-Path $directory.FullName 'x64/Microsoft.VC143.CRT'
                    if (Test-Path -LiteralPath $candidate) { $RuntimeDirectory = $candidate; break }
                }
            }
        }
    }
    if (!$RuntimeDirectory) { throw 'Visual C++ x64 redistributable DLLs were not found. Pass -RuntimeDirectory from a Visual Studio VC/Redist/MSVC installation.' }
    $RuntimeDirectory = (Resolve-Path -LiteralPath $RuntimeDirectory).Path
    foreach ($name in @('msvcp140.dll', 'vcruntime140.dll', 'vcruntime140_1.dll')) {
        if (!(Test-Path -LiteralPath (Join-Path $RuntimeDirectory $name))) { throw "Runtime DLL is missing: $name" }
    }

    New-Item -ItemType Directory -Force -Path dist | Out-Null
    & $IsccPath "/DAppVersion=$version" "/DBinaryPath=$BinaryPath" "/DRuntimeDir=$RuntimeDirectory" packaging/windows/keyframe.iss
    if ($LASTEXITCODE -ne 0) { throw 'The Windows installer build failed.' }
    Get-Item -LiteralPath dist/keyframe-windows-x64-setup.exe
} finally {
    Pop-Location
}
