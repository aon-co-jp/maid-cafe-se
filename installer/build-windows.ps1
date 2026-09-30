# maid-cafe-se Windows版インストーラー(maid-cafe-se-installer.exe)のビルド。 / Build the Windows installer.
#
# 使い方 / Usage:
#   powershell -ExecutionPolicy Bypass -File installer\build-windows.ps1
# 必要なもの / Requires: Rust (cargo, MSVC), NSIS 3 (makensis)。JDKは不要(v0.5.0からRust製)。
# 出力 / Output: installer\dist\maid-cafe-se-installer.exe と SHA256SUMS.txt への追記(distはgit管理外)
param(
    [string]$Makensis = "C:\Program Files (x86)\NSIS\makensis.exe",
    [switch]$SkipSelfTest
)
$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
$cargoBin = Join-Path $env:USERPROFILE '.cargo\bin'
if (Test-Path $cargoBin) { $env:PATH = "$cargoBin;$env:PATH" }
if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) { throw "cargoが見つかりません(Rustを入れてください) / cargo not found" }
if (-not (Test-Path $Makensis)) { throw "makensisが見つかりません(NSIS 3を入れてください) / makensis not found: $Makensis" }

$toml = Get-Content (Join-Path $root 'crates\maid-cafe-desktop\Cargo.toml') -Raw
if ($toml -notmatch '(?m)^version\s*=\s*"([^"]+)"') { throw "versionが読めません / cannot read version" }
$version = $Matches[1]

Push-Location $root
try {
    cargo test -p maid-cafe-core -p maid-cafe-desktop
    if ($LASTEXITCODE -ne 0) { throw "テスト失敗 / tests failed" }
    cargo build --release -p maid-cafe-desktop
    if ($LASTEXITCODE -ne 0) { throw "ビルド失敗 / build failed" }
} finally { Pop-Location }

# 配布物のフォルダ(exe+アイコン)。exeは単体で動く(音源は埋め込み済み、JREなど不要)
$appDir = Join-Path $root 'installer\build\app'
Remove-Item $appDir -Recurse -Force -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force $appDir | Out-Null
$exe = Join-Path $appDir 'maid-cafe-se.exe'
Copy-Item (Join-Path $root 'target\release\maid-cafe-se.exe') $exe
Copy-Item (Join-Path $root 'crates\maid-cafe-desktop\assets\maid-cafe-se.ico') $appDir

# 配布物の自己診断(Windows音声→加工→WAV書き出し)。日本語音声が無いPCでは失敗するので -SkipSelfTest で省ける
if (-not $SkipSelfTest) {
    $st = Join-Path $root 'installer\build\selftest'
    Remove-Item $st -Recurse -Force -ErrorAction SilentlyContinue
    $p = Start-Process $exe -ArgumentList '--selftest', "`"$st`"" -PassThru -Wait
    Get-Content (Join-Path $st 'report.txt') -ErrorAction SilentlyContinue
    if ($p.ExitCode -ne 0) { throw "自己診断に失敗 / selftest failed (exit $($p.ExitCode))" }
    Write-Host "selftest OK"
}

$dist = Join-Path $PSScriptRoot 'dist'
New-Item -ItemType Directory -Force $dist | Out-Null
$out = Join-Path $dist 'maid-cafe-se-installer.exe'
& $Makensis /V2 "/DVERSION=$version" "/DAPPDIR=$appDir" "/DOUTFILE=$out" (Join-Path $PSScriptRoot 'windows\installer.nsi')
if ($LASTEXITCODE -ne 0) { throw "makensis失敗 / makensis failed" }

$hash = (Get-FileHash $out -Algorithm SHA256).Hash.ToLower()
$sums = Join-Path $dist 'SHA256SUMS.txt'
$line = "$hash  maid-cafe-se-installer.exe"
if (Test-Path $sums) {
    $keep = Get-Content $sums | Where-Object { $_ -notmatch 'maid-cafe-se-installer\.exe' }
    @($keep) + $line | Set-Content $sums -Encoding ascii
} else { $line | Set-Content $sums -Encoding ascii }
Write-Host ("OK: {0} ({1:N1} MB)" -f $out, ((Get-Item $out).Length / 1MB))
Write-Host "SHA256: $hash"
