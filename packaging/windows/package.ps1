param(
    [string]$OutputDirectory = "dist/windows"
)

$ErrorActionPreference = "Stop"
$root = (Resolve-Path (Join-Path $PSScriptRoot "../..")).Path
$output = Join-Path $root $OutputDirectory
$stage = Join-Path $output "Kog"
$version = ((Select-String -Path (Join-Path $root "Cargo.toml") -Pattern '^version = "([^"]+)"').Matches[0].Groups[1].Value)

Remove-Item -Recurse -Force $output -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force -Path $stage | Out-Null
Copy-Item (Join-Path $root "target/release/kog.exe") (Join-Path $stage "Kog.exe")
Copy-Item (Join-Path $root "LICENSE") $stage

$helpers = @(
    "kog-sfm-helper.exe", "kog-psf-helper.exe", "kog-psf2-helper.exe",
    "kog-2sf-helper.exe", "kog-snsf-helper.exe", "kog-syntrax-helper.exe",
    "kog-sc55-helper.exe"
)
foreach ($helper in $helpers) {
    $candidate = Get-ChildItem (Join-Path $root "target/release/build") -Recurse -File -Filter $helper |
        Where-Object { $_.DirectoryName -match '[\\/]bin$' } |
        Select-Object -First 1
    if (-not $candidate) {
        throw "Missing release helper: $helper"
    }
    Copy-Item $candidate.FullName (Join-Path $stage $helper)
}

if ($env:VCPKG_ROOT) {
    $vcpkgBin = Join-Path $env:VCPKG_ROOT "installed/x64-windows/bin"
    if (Test-Path $vcpkgBin) {
        Copy-Item (Join-Path $vcpkgBin "*.dll") $stage
    }
}

$deployQt = (Get-Command windeployqt.exe).Source
& $deployQt --release --compiler-runtime --qmldir (Join-Path $root "qml") (Join-Path $stage "Kog.exe")
if ($LASTEXITCODE -ne 0) {
    throw "windeployqt failed"
}

$process = Start-Process -FilePath (Join-Path $stage "Kog.exe") -PassThru
Start-Sleep -Seconds 5
if ($process.HasExited) {
    throw "Packaged Kog exited during the Windows launch smoke test with code $($process.ExitCode)"
}
Stop-Process -Id $process.Id -Force

$zip = Join-Path $output "Kog-$version-windows-x86_64-portable.zip"
Compress-Archive -Path (Join-Path $stage "*") -DestinationPath $zip -CompressionLevel Optimal

$wixBin = if ($env:WIX) { Join-Path $env:WIX "bin" } else { "C:\Program Files (x86)\WiX Toolset v3.14\bin" }
$heat = Join-Path $wixBin "heat.exe"
$candle = Join-Path $wixBin "candle.exe"
$light = Join-Path $wixBin "light.exe"
foreach ($tool in @($heat, $candle, $light)) {
    if (-not (Test-Path $tool)) { throw "WiX tool not found: $tool" }
}

$harvest = Join-Path $output "KogFiles.wxs"
& $heat dir $stage -cg KogFiles -dr INSTALLFOLDER -srd -sfrag -gg -var var.SourceDir -out $harvest
if ($LASTEXITCODE -ne 0) { throw "WiX harvesting failed" }
& $candle -nologo -arch x64 "-dSourceDir=$stage" "-dVersion=$version" -out "$output\" (Join-Path $root "packaging/windows/Kog.wxs") $harvest
if ($LASTEXITCODE -ne 0) { throw "WiX compilation failed" }
& $light -nologo -sice:ICE61 -out (Join-Path $output "Kog-$version-windows-x86_64.msi") (Join-Path $output "Kog.wixobj") (Join-Path $output "KogFiles.wixobj")
if ($LASTEXITCODE -ne 0) { throw "WiX linking failed" }
