$ErrorActionPreference = "Stop"

$root = Split-Path -Parent $PSScriptRoot
Set-Location $root

$manifest = Get-Content (Join-Path $root "Cargo.toml") -Raw
$versionMatch = [regex]::Match($manifest, '(?m)^version = "(\d+\.\d+\.\d+)"')
if (-not $versionMatch.Success) {
    throw "无法从 Cargo.toml 读取版本号"
}
$version = $versionMatch.Groups[1].Value

function Invoke-Checked {
    param(
        [Parameter(Mandatory = $true)]
        [scriptblock] $Command,
        [Parameter(Mandatory = $true)]
        [string] $Description
    )

    & $Command
    if ($LASTEXITCODE -ne 0) {
        throw "$Description 失败，退出码：$LASTEXITCODE"
    }
}

Write-Host "==> cargo fmt --check"
Invoke-Checked { cargo fmt --all -- --check } "代码格式检查"

Write-Host "==> cargo clippy"
Invoke-Checked { cargo clippy --all-targets -- -D warnings } "Clippy 检查"

Write-Host "==> cargo test"
Invoke-Checked { cargo test --all-targets } "单元测试"

Write-Host "==> cargo build --release"
Invoke-Checked { cargo build --release --bins } "Release 构建"

$stageName = "AnyLink-v$version-Windows-x64"
$stage = Join-Path $root "dist\$stageName"
if (Test-Path $stage) {
    Remove-Item $stage -Recurse -Force
}
New-Item -ItemType Directory -Force -Path $stage | Out-Null
Copy-Item (Join-Path $root "target\release\anylink.exe") $stage
Copy-Item (Join-Path $root "target\release\anylink-cli.exe") $stage
Copy-Item (Join-Path $root "README.md") $stage
Copy-Item (Join-Path $root "CHANGELOG.md") $stage
Copy-Item (Join-Path $root "docs") $stage -Recurse

$zip = Join-Path $root "dist\$stageName.zip"
if (Test-Path $zip) {
    Remove-Item $zip -Force
}
Compress-Archive -Path (Join-Path $stage "*") -DestinationPath $zip -CompressionLevel Optimal

$hash = (Get-FileHash $zip -Algorithm SHA256).Hash
$hashFile = Join-Path $root "dist\$stageName.sha256.txt"
"$hash  $stageName.zip" | Set-Content -Path $hashFile -Encoding ascii

Write-Host ""
Write-Host "构建完成："
Write-Host "  AnyLink 版本：v$version"
Write-Host "  $stage\anylink.exe"
Write-Host "  $stage\anylink-cli.exe"
Write-Host "  $zip"
Write-Host "  $hashFile"
Write-Host "  SHA256：$hash"
