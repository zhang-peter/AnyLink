param(
    [Parameter(Mandatory = $true)]
    [ValidatePattern('^\d+\.\d+\.\d+$')]
    [string] $Version
)

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
$manifest = Join-Path $root "Cargo.toml"
$content = Get-Content $manifest -Raw

if ($content -notmatch '(?m)^version = "\d+\.\d+\.\d+"') {
    throw "Cargo.toml 中找不到 package version"
}

$content = [regex]::Replace(
    $content,
    '(?m)^version = "\d+\.\d+\.\d+"',
    "version = `"$Version`""
)
[System.IO.File]::WriteAllText($manifest, $content, [System.Text.UTF8Encoding]::new($false))

Push-Location $root
try {
    cargo check --quiet
} finally {
    Pop-Location
}

Write-Host "AnyLink 已更新到 v$Version"
Write-Host "请同步更新 CHANGELOG.md，然后运行 scripts\\build.ps1 生成版本化发布包。"
