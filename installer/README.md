# installer/

maid-cafe-se のインストーラー(配布物)のビルド元と、入手・インストール方法。
**バイナリ自体はこのフォルダにコミットしません**(`dist/`はgit管理外)。配布の正本は
[GitHub Releases](https://github.com/aon-co-jp/maid-cafe-se/releases) です
(バイナリをgitに入れるとリポジトリが肥大化し続け、`git clone`が遅くなるため)。

Build scripts and install instructions for maid-cafe-se. **Binaries are not committed here** (`dist/` is git-ignored);
[GitHub Releases](https://github.com/aon-co-jp/maid-cafe-se/releases) is the canonical download location.

## ダウンロード / Download

最新版 / Latest: <https://github.com/aon-co-jp/maid-cafe-se/releases/latest>

| プラットフォーム / Platform | ファイル / File | 内容 / Notes |
|---|---|---|
| **Windows** 10/11 (x64) | `maid-cafe-se-installer.exe` | インストーラー(実行環境同梱、Java不要)。管理者権限不要でユーザー領域にインストール |
| **Android** 8.0+ | `maid-cafe-se_<version>_android.apk` | 署名つきAPK(サイドロード)。PCから入れるスクリプトも用意 |
| 検証 / Verify | `SHA256SUMS.txt` | 各ファイルのSHA-256 |

## Windows版のインストール / Install on Windows

1. `maid-cafe-se-installer.exe` をダブルクリックして、画面の案内に従う(日本語/英語)。
   - このインストーラーはコード署名していないため、Windows SmartScreen が「PCが保護されました」と出すことがあります。
     その場合は **「詳細情報」→「実行」** を選んでください。
   - 無人インストール: `maid-cafe-se-installer.exe /S`(インストール先を変える: `/D=C:\任意のフォルダ`、`/D=`は必ず最後に)。
2. スタートメニュー/デスクトップの「maid-cafe-se」から起動します。ウィンドウを閉じても**タスクトレイに常駐**して、時刻になると鳴ります(完全に止めるにはトレイのアイコンから「終了」)。
3. Windowsの起動時に自動で起動したい場合は、アプリ内のチェックボックスをオンにします。
4. 読み上げには **日本語のWindows音声** が必要です(標準の「Microsoft Haruka」など)。無い場合は
   「設定 → 時刻と言語 → 言語と地域」で日本語を追加し、音声を入れてください。
5. アンインストールは「設定 → アプリ」から。アラームなどの設定(`%APPDATA%\maid-cafe-se`)はあなたのデータなので**残ります**。

## Android版のインストール / Install on Android

### 方法A: 端末で直接
1. 端末のブラウザで [Releases](https://github.com/aon-co-jp/maid-cafe-se/releases/latest) の `maid-cafe-se_<version>_android.apk` をダウンロードして開く。
2. 「提供元不明のアプリ」のインストール許可を求められたら、そのブラウザ/ファイラーに対して許可する。

### 方法B: PCから(USB接続)
1. 端末の「開発者向けオプション」→「USBデバッグ」をオンにしてPCにつなぎ、端末に出る許可ダイアログを承認。
2. `install-android.bat` をダブルクリック(APKがこのフォルダに無ければ、最新リリースを自動でダウンロードしてSHA-256を検証)。
   - 要: [Android SDK Platform-Tools](https://developer.android.com/tools/releases/platform-tools)(`adb`)。
   - 旧バージョン(**v0.3.1以前のデバッグ署名版**)が入っていると署名が違うので上書きできません。
     設定とアラームが消えてよければ `install-android.bat -ReplaceOldSignature` を使ってください。

署名の証明書 SHA-256 / Signing certificate SHA-256:
`104cd63b22a418b03d72a62708d529399a050bf948f5ef3d11e0a4dfd4ea12af`

`apksigner verify --print-certs <apk>` の「certificate SHA-256 digest」がこの値と一致すれば、正規のリリースです。

## ビルドの仕組み / How it is built

すべてローカルのPowerShellスクリプトでビルドします(署名鍵をリポジトリ・CIに置かないため)。

| スクリプト | 役割 |
|---|---|
| `build-release.ps1` | Android版の署名つきAPKを作る(`installer\dist\maid-cafe-se_<version>_android.apk`)。鍵情報は `F:\maid-cafe-se-keystore.txt`(**リポジトリの外**、`-KeystoreInfo`で変更可)から環境変数経由でGradleへ渡し、画面・ログに出さない。`apksigner verify`で検証 |
| `build-windows.ps1` | Windows版を`jpackage`で実行環境つきにまとめ(自己診断 `--selftest` を実行)、NSISで `maid-cafe-se-installer.exe` を作る。要: JDK 17(jpackage入り)、NSIS 3 |
| `windows/installer.nsi` | NSISのインストーラー定義(ユーザー領域へインストール、ショートカット、アンインストーラー、日本語/英語) |
| `install-android.ps1` / `.bat` | PCから端末へAPKを入れるスクリプト(上記 方法B) |

```powershell
powershell -ExecutionPolicy Bypass -File installer\build-release.ps1   # Android
powershell -ExecutionPolicy Bypass -File installer\build-windows.ps1   # Windows
```

**署名鍵について**: 鍵(`F:\maid-cafe-se-release.jks`)を失うと、以後のAndroid版アップデートが既存のインストールに上書きできなくなります。バックアップを取り、絶対にコミットしないでください。
