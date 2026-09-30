# maid-cafe-se 正式リリースAPKのビルド(署名つき)。 / Build the signed release APK.
#
# 使い方 / Usage:
#   powershell -ExecutionPolicy Bypass -File installer\build-release.ps1
#   (鍵情報ファイルを変える場合 / custom key info file: -KeystoreInfo <path>)
#
# 鍵情報ファイル(既定 F:\maid-cafe-se-keystore.txt、リポジトリの外に置く。絶対にコミットしない)の形式:
#   KEYSTORE_FILE=...\maid-cafe-se-release.jks
#   KEYSTORE_PASSWORD=...
#   KEY_ALIAS=maid-cafe-se
#   KEY_PASSWORD=...
# 出力: installer\dist\maid-cafe-se_<version>_android.apk と SHA256SUMS.txt(distはgit管理外)
param(
    [string]$KeystoreInfo = "F:\maid-cafe-se-keystore.txt",
    [string]$JavaHome = "C:\Program Files\Android\Android Studio\jbr"
)
$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot

if (-not (Test-Path $KeystoreInfo)) { throw "鍵情報ファイルがありません / key info file not found: $KeystoreInfo" }
$info = @{}
Get-Content $KeystoreInfo | ForEach-Object { if ($_ -match '^([A-Z_]+)=(.*)$') { $info[$Matches[1]] = $Matches[2] } }
foreach ($k in 'KEYSTORE_FILE', 'KEYSTORE_PASSWORD', 'KEY_ALIAS', 'KEY_PASSWORD') {
    if (-not $info[$k]) { throw "鍵情報に $k がありません / missing $k in $KeystoreInfo" }
}
if (-not (Test-Path $info['KEYSTORE_FILE'])) { throw "keystoreがありません / keystore not found: $($info['KEYSTORE_FILE'])" }

# 鍵情報は環境変数でGradleへ渡す(画面・ログには出さない)
$env:MAID_CAFE_SE_KEYSTORE_FILE = $info['KEYSTORE_FILE']
$env:MAID_CAFE_SE_KEYSTORE_PASSWORD = $info['KEYSTORE_PASSWORD']
$env:MAID_CAFE_SE_KEY_ALIAS = $info['KEY_ALIAS']
$env:MAID_CAFE_SE_KEY_PASSWORD = $info['KEY_PASSWORD']
if (Test-Path $JavaHome) { $env:JAVA_HOME = $JavaHome }

$gradle = Get-Content (Join-Path $root 'app\build.gradle.kts') -Raw
if ($gradle -notmatch 'versionName\s*=\s*"([^"]+)"') { throw "versionNameが読めません / cannot read versionName" }
$version = $Matches[1]

Push-Location $root
try {
    & "$root\gradlew.bat" ':core:test' ':app:assembleRelease' '--console=plain'
    if ($LASTEXITCODE -ne 0) { throw "ビルド/テスト失敗 / build or tests failed" }
} finally { Pop-Location }

$built = Join-Path $root 'app\build\outputs\apk\release\app-release.apk'
if (-not (Test-Path $built)) { throw "署名つきAPKができていません / signed APK not produced: $built" }

$dist = Join-Path $PSScriptRoot 'dist'
New-Item -ItemType Directory -Force $dist | Out-Null
$out = Join-Path $dist "maid-cafe-se_${version}_android.apk"
Copy-Item $built $out -Force

# 署名の検証(apksignerがあれば)
$sdk = if ($env:ANDROID_HOME) { $env:ANDROID_HOME } else { Join-Path $env:LOCALAPPDATA 'Android\Sdk' }
$apksigner = Get-ChildItem (Join-Path $sdk 'build-tools') -Recurse -Filter apksigner.bat -ErrorAction SilentlyContinue |
    Sort-Object FullName -Descending | Select-Object -First 1
if ($apksigner) {
    & $apksigner.FullName verify --print-certs $out
    if ($LASTEXITCODE -ne 0) { throw "署名の検証に失敗 / signature verification failed" }
} else {
    Write-Warning "apksignerが見つからないため署名検証をスキップ / apksigner not found, skipping verification"
}

$hash = (Get-FileHash $out -Algorithm SHA256).Hash.ToLower()
# SHA256SUMS.txtは、他の成果物(Windows版インストーラー等)の行を消さずに、このAPKの行だけ更新する
$sums = Join-Path $dist 'SHA256SUMS.txt'
$name = Split-Path -Leaf $out
$keep = if (Test-Path $sums) { Get-Content $sums | Where-Object { $_ -notmatch '_android\.apk' } } else { @() }
@($keep) + "$hash  $name" | Set-Content $sums -Encoding ascii
Write-Host "OK: $out"
Write-Host "SHA256: $hash"
