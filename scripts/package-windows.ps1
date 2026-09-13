# Package Windows CLI artifacts: portable ZIP + SHA256 checksums.
$ErrorActionPreference = "Stop"

$scriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$root = Split-Path -Parent $scriptDir
$dist = Join-Path $root "dist"
$stage = Join-Path $dist "tdx-doc-windows-x86_64"
$version = "0.1.0"

Push-Location $root
try {
    cargo build --release -p tdx-cli

    if (Test-Path $stage) { Remove-Item -Recurse -Force $stage }
    New-Item -ItemType Directory -Force -Path $stage | Out-Null

    Copy-Item (Join-Path $root "target\release\tdx-doc.exe") (Join-Path $stage "tdx-doc.exe")
    Copy-Item (Join-Path $root "LICENSE") (Join-Path $stage "LICENSE")
    Copy-Item (Join-Path $root "README.md") (Join-Path $stage "README.md")
    Copy-Item (Join-Path $root "THIRD_PARTY_LICENSES.md") (Join-Path $stage "THIRD_PARTY_LICENSES.md")

    $zip = Join-Path $dist "tdx-doc-windows-x86_64.zip"
    if (Test-Path $zip) { Remove-Item -Force $zip }
    Compress-Archive -Path (Join-Path $stage "*") -DestinationPath $zip

    $lines = @()
    foreach ($file in @($zip, (Join-Path $stage "tdx-doc.exe"))) {
        $hash = (Get-FileHash -Algorithm SHA256 -Path $file).Hash.ToLower()
        $name = Split-Path -Leaf $file
        $lines += "$hash  $name"
    }
    Set-Content -Path (Join-Path $dist "SHA256SUMS.txt") -Value $lines -Encoding ASCII

    Write-Host "Artifacts in $dist"
    Get-ChildItem $dist | Select-Object Name, Length
}
finally {
    Pop-Location
}
