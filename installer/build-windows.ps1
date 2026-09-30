# maid-cafe-se Windows版インストーラー(maid-cafe-se-installer.exe)のビルド。 / Build the Windows installer.
#
# 使い方 / Usage:
#   powershell -ExecutionPolicy Bypass -File installer\build-windows.ps1
# 必要なもの / Requires: JDK 17 (jpackage入り), NSIS 3 (makensis), Android Studio同梱JBR等のGradle用JDK
# 出力 / Output: installer\dist\maid-cafe-se-installer.exe と SHA256SUMS.txt への追記(distはgit管理外)
param(
    [string]$JavaHome = "C:\Program Files\Android\Android Studio\jbr",   # Gradleを動かすJDK
    [string]$JpackageJdk = "C:\Program Files\Java\jdk-17",               # jpackageを含むJDK
    [string]$Makensis = "C:\Program Files (x86)\NSIS\makensis.exe",
    [switch]$SkipSelfTest
)
$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
if (-not (Test-Path (Join-Path $JpackageJdk 'bin\jpackage.exe'))) { throw "jpackage.exeが見つかりません / jpackage not found in $JpackageJdk" }
if (-not (Test-Path $Makensis)) { throw "makensisが見つかりません(NSIS 3を入れてください) / makensis not found: $Makensis" }
if (Test-Path $JavaHome) { $env:JAVA_HOME = $JavaHome }
$env:MAID_CAFE_SE_JPACKAGE_JDK = $JpackageJdk

$gradle = Get-Content (Join-Path $root 'desktop\build.gradle.kts') -Raw
if ($gradle -notmatch 'packageVersion\s*=\s*"([^"]+)"') { throw "packageVersionが読めません / cannot read packageVersion" }
$version = $Matches[1]

Push-Location $root
try {
    & "$root\gradlew.bat" ':core:test' ':desktop:createDistributable' '--console=plain'
    if ($LASTEXITCODE -ne 0) { throw "ビルド/テスト失敗 / build or tests failed" }
} finally { Pop-Location }

$appDir = Join-Path $root 'desktop\build\compose\binaries\main\app\maid-cafe-se'
$exe = Join-Path $appDir 'maid-cafe-se.exe'
if (-not (Test-Path $exe)) { throw "配布物ができていません / distributable not produced: $exe" }

# 配布物の自己診断(Windows音声→加工→WAV書き出し)。日本語音声が無いPCではスキップ/失敗するので -SkipSelfTest で省ける
if (-not $SkipSelfTest) {
    $st = Join-Path $root 'desktop\build\selftest-build'
    Remove-Item $st -Recurse -Force -ErrorAction SilentlyContinue
    $p = Start-Process $exe -ArgumentList '--selftest', $st -PassThru -Wait
    if ($p.ExitCode -ne 0) { Get-Content (Join-Path $st 'report.txt') -ErrorAction SilentlyContinue; throw "自己診断に失敗 / selftest failed (exit $($p.ExitCode))" }
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
