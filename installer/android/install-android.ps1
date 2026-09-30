# maid-cafe-se をUSB接続したAndroid端末へインストールする(PC用インストーラー)。
# Installs maid-cafe-se onto a USB-connected Android phone from this PC.
#
# 使い方 / Usage (端末の「開発者向けオプション」→「USBデバッグ」をオンにして接続 / enable USB debugging and connect):
#   install-android.bat            ... mobile/(またはこのフォルダ・dist)のAPK、無ければ最新リリースを取得
#   install-android.bat -Apk <path> ... 指定のAPKを入れる
#   install-android.bat -Serial <id> ... 複数台つながっているとき、対象の端末
#   install-android.bat -ReplaceOldSignature ... 旧デバッグ署名版が入っていて競合するとき、いったんアンインストールして入れ直す(設定・アラームは消える)
param(
    [string]$Apk = "",
    [string]$Serial = "",
    [switch]$ReplaceOldSignature
)
$ErrorActionPreference = "Stop"
$package = "tokyo.runo.maidcafese"
$repo = "aon-co-jp/maid-cafe-se"

# --- APKを決める: 引数 → mobile → このフォルダ → dist → 最新リリースからダウンロード ---
if (-not $Apk) {
    $local = @((Join-Path $PSScriptRoot 'mobile'), $PSScriptRoot, (Join-Path (Split-Path -Parent $PSScriptRoot) 'dist')) |
        ForEach-Object { Get-ChildItem $_ -Filter 'maid-cafe-se_*_android.apk' -ErrorAction SilentlyContinue } |
        Sort-Object Name -Descending | Select-Object -First 1
    if ($local) {
        $Apk = $local.FullName
    } else {
        Write-Host "APKが見つからないので最新リリースを取得します / downloading the latest release..."
        $rel = Invoke-RestMethod "https://api.github.com/repos/$repo/releases/latest" -Headers @{ 'User-Agent' = 'maid-cafe-se-installer' }
        $asset = $rel.assets | Where-Object { $_.name -like 'maid-cafe-se_*_android.apk' } | Select-Object -First 1
        if (-not $asset) { throw "最新リリースにAPKがありません / no APK in the latest release" }
        $Apk = Join-Path $env:TEMP $asset.name
        Invoke-WebRequest $asset.browser_download_url -OutFile $Apk -UseBasicParsing
        $sums = $rel.assets | Where-Object { $_.name -eq 'SHA256SUMS.txt' } | Select-Object -First 1
        if ($sums) {
            $expected = ((Invoke-RestMethod $sums.browser_download_url) -split "`n" | Where-Object { $_ -match [regex]::Escape($asset.name) }) -replace '\s.*', ''
            $actual = (Get-FileHash $Apk -Algorithm SHA256).Hash.ToLower()
            if ($expected -and $expected.Trim().ToLower() -ne $actual) { throw "SHA256が一致しません / SHA256 mismatch (expected $expected, got $actual)" }
            Write-Host "SHA256 OK"
        }
    }
}
if (-not (Test-Path $Apk)) { throw "APKがありません / APK not found: $Apk" }
Write-Host "APK: $Apk"

# --- adbを探す ---
$adb = (Get-Command adb -ErrorAction SilentlyContinue).Source
if (-not $adb) {
    foreach ($base in @($env:ANDROID_HOME, $env:ANDROID_SDK_ROOT, (Join-Path $env:LOCALAPPDATA 'Android\Sdk'))) {
        if ($base -and (Test-Path (Join-Path $base 'platform-tools\adb.exe'))) { $adb = Join-Path $base 'platform-tools\adb.exe'; break }
    }
}
if (-not $adb) {
    throw "adbが見つかりません。Android SDK Platform-Tools(https://developer.android.com/tools/releases/platform-tools)を入れてPATHを通してください / adb not found: install Android SDK Platform-Tools"
}


# adbを呼ぶ。Windows PowerShell 5.1は、ネイティブコマンドの標準エラー出力(adbは警告も出す)を
# エラー扱いして$ErrorActionPreference=Stopで止まるため、呼び出しの間だけ止まらないようにして出力を返す。
function Invoke-Adb {
    $prev = $ErrorActionPreference
    $ErrorActionPreference = "Continue"
    try { (& $script:adb @args 2>&1 | ForEach-Object { "$_" }) -join "`n" } finally { $ErrorActionPreference = $prev }
}

# --- 端末を決める ---
$devices = @((Invoke-Adb devices) -split "`n" | Select-Object -Skip 1 | Where-Object { $_ -match '\tdevice$' } | ForEach-Object { ($_ -split "`t")[0] })
if ($Serial) {
    if ($devices -notcontains $Serial) { throw "端末 $Serial が見つかりません / device $Serial not found" }
} elseif ($devices.Count -eq 1) {
    $Serial = $devices[0]
} elseif ($devices.Count -eq 0) {
    throw "端末が見つかりません。USBデバッグをオンにして接続し、端末側の許可ダイアログを承認してください / no device: enable USB debugging and accept the prompt on the phone"
} else {
    throw "端末が複数あります。-Serial で指定してください / multiple devices, use -Serial: $($devices -join ', ')"
}
Write-Host "端末 / device: $Serial"

# --- インストール ---
$result = Invoke-Adb -s $Serial install -r $Apk
if ($result -match 'INSTALL_FAILED_UPDATE_INCOMPATIBLE|signatures do not match') {
    if (-not $ReplaceOldSignature) {
        throw "署名が異なる旧版(デバッグ署名のv0.3.1以前など)が入っています。設定とアラームが消えてよければ -ReplaceOldSignature を付けて再実行してください / an older build signed with a different key is installed; re-run with -ReplaceOldSignature (this removes its data)"
    }
    Write-Host "旧版をアンインストールします / uninstalling the old build..."
    Invoke-Adb -s $Serial uninstall $package | Out-Null
    $result = Invoke-Adb -s $Serial install $Apk
}
if ($result -notmatch 'Success') { throw "インストール失敗 / install failed:`n$result" }
Write-Host "インストール完了 / installed"

# --- 通知権限(Android 13以降)を付与して起動 ---
$sdkInt = [int]((Invoke-Adb -s $Serial shell getprop ro.build.version.sdk) -replace '\D', '')
if ($sdkInt -ge 33) { Invoke-Adb -s $Serial shell pm grant $package android.permission.POST_NOTIFICATIONS | Out-Null }
Invoke-Adb -s $Serial shell am start -n "$package/.MainActivity" | Out-Null
Write-Host "起動しました。カレンダー連動を使う場合はアプリ内で権限を許可してください / launched. Grant calendar access inside the app if you use calendar integration."
