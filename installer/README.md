# installer/

maid-cafe-se のインストーラー(配布物)のビルド元と、入手・インストール方法。
**Windows版のインストーラー(約1.5MB)だけは`windows/`にコミットします**(フォルダから直接ダウンロードできるように。APKなど大きいものは`dist/`=git管理外)。配布の正本は
[GitHub Releases](https://github.com/aon-co-jp/maid-cafe-se/releases) です
(バイナリをgitに入れるとリポジトリが肥大化し続け、`git clone`が遅くなるため)。

Build scripts and install instructions for maid-cafe-se. **Binaries are not committed here** (`dist/` is git-ignored);
[GitHub Releases](https://github.com/aon-co-jp/maid-cafe-se/releases) is the canonical download location.

## ダウンロード / Download

最新版 / Latest: <https://github.com/aon-co-jp/maid-cafe-se/releases/latest>

| プラットフォーム / Platform | ファイル / File | 内容 / Notes |
|---|---|---|
| **Windows** 10/11 (x64) | `maid-cafe-se-installer.exe` | インストーラー(Rust製の単体exe、Java不要)。管理者権限不要でユーザー領域にインストール |
| **Android** 8.0+ | `maid-cafe-se_<version>_android.apk`(フォルダ: `android/mobile/`) | 署名つきAPK(サイドロード)。PCから入れるスクリプトも用意 |
| 検証 / Verify | `SHA256SUMS.txt` | 各ファイルのSHA-256 |

## Windows版のインストール / Install on Windows

1. `maid-cafe-se-installer.exe` をダブルクリックして、画面の案内に従う(日本語/英語)。
   - このインストーラーはコード署名していないため、Windows SmartScreen が「PCが保護されました」と出すことがあります。
     その場合は **「詳細情報」→「実行」** を選んでください。
   - 無人インストール: `maid-cafe-se-installer.exe /S`(インストール先を変える: `/D=C:\任意のフォルダ`、`/D=`は必ず最後に)。
2. スタートメニュー/デスクトップの「maid-cafe-se」から起動します。ウィンドウを閉じても**タスクトレイに常駐**して、時刻になると鳴ります(完全に止めるにはトレイのアイコンから「終了」)。
3. Windowsの起動時に自動で起動したい場合は、画面右上の「設定」で、起動時の自動起動をオンにします。
4. 読み上げには **日本語のWindows音声** が必要です(標準の「Microsoft Haruka」など)。無い場合は
   「設定 → 時刻と言語 → 言語と地域」で日本語を追加し、音声を入れてください。
5. アンインストールは「設定 → アプリ」から。アラームなどの設定(`%APPDATA%\maid-cafe-se`)はあなたのデータなので**残ります**。

## Android版のインストール / Install on Android

### 方法A: 端末で直接
1. 端末のブラウザで [Releases](https://github.com/aon-co-jp/maid-cafe-se/releases/latest) の `maid-cafe-se_<version>_android.apk` をダウンロードして開く。
2. 「提供元不明のアプリ」のインストール許可を求められたら、そのブラウザ/ファイラーに対して許可する。

### 方法B: PCから(USB接続)
1. 端末の「開発者向けオプション」→「USBデバッグ」をオンにしてPCにつなぎ、端末に出る許可ダイアログを承認。
2. `android/install-android.bat` をダブルクリック(APKが`android/mobile/`に無ければ、最新リリースを自動でダウンロードしてSHA-256を検証)。
   - 要: [Android SDK Platform-Tools](https://developer.android.com/tools/releases/platform-tools)(`adb`)。
   - 旧バージョン(**v0.3.1以前のデバッグ署名版**)が入っていると署名が違うので上書きできません。
     設定とアラームが消えてよければ `android/install-android.bat -ReplaceOldSignature` を使ってください。

署名の証明書 SHA-256 / Signing certificate SHA-256:
`104cd63b22a418b03d72a62708d529399a050bf948f5ef3d11e0a4dfd4ea12af`

`apksigner verify --print-certs <apk>` の「certificate SHA-256 digest」がこの値と一致すれば、正規のリリースです。

## ビルドの仕組み / How it is built

すべてローカルのPowerShellスクリプトでビルドします(署名鍵をリポジトリ・CIに置かないため)。

| スクリプト | 役割 |
|---|---|
| `build-release.ps1` | Android版の署名つきAPKを作る(`installer\dist\maid-cafe-se_<version>_android.apk`)。鍵情報は `F:\maid-cafe-se-keystore.txt`(**リポジトリの外**、`-KeystoreInfo`で変更可)から環境変数経由でGradleへ渡し、画面・ログに出さない。`apksigner verify`で検証 |
| `build-windows.ps1` | Windows版(Rust、`crates/maid-cafe-desktop`)のテスト→リリースビルド→自己診断 `--selftest` を実行し、NSISで `maid-cafe-se-installer.exe` を作る(約1.5MB、JRE不要)。要: Rust、NSIS 3 |
| `make-icon.ps1` | アプリのアイコン(`crates/maid-cafe-desktop/assets/maid-cafe-se.ico`)を生成する。通常は不要(生成済みをコミット) |
| `nsis-installer.nsi` | NSISのインストーラー定義=exeを作るための設計図(ユーザー領域へインストール、ショートカット、アンインストーラー、日本語/英語) |
| `windows/maid-cafe-se-installer.exe` | 完成したWindows版インストーラー(約1.5MB)。フォルダからそのままダウンロードできる。最新版は常にここと[Releases](https://github.com/aon-co-jp/maid-cafe-se/releases)に同じものを置く |
| `android/install-android.ps1` / `.bat` | PCから端末へAPKを入れるスクリプト(上記 方法B) |

```powershell
powershell -ExecutionPolicy Bypass -File installer\build-release.ps1   # Android
powershell -ExecutionPolicy Bypass -File installer\build-windows.ps1   # Windows
```

**署名鍵について**: 鍵(`F:\maid-cafe-se-release.jks`)を失うと、以後のAndroid版アップデートが既存のインストールに上書きできなくなります。バックアップを取り、絶対にコミットしないでください。

## フォルダの構成 / Layout

```
installer/                                      (master: スクリプトと説明だけ。容量を増やさないため、バイナリは入れない)
├── windows/                                    手元でビルドしたexeの出力先(コミット対象外)+README
├── android/
│   ├── mobile/                                 手元でビルドしたAPKの出力先(コミット対象外)+README
│   └── install-android.ps1 / .bat              PCからUSBで入れるスクリプト
├── nsis-installer.nsi                          Windowsインストーラーの設計図(makensisでexeにする)
├── build-windows.ps1 / build-release.ps1 / make-icon.ps1
└── dist/                                       ビルドの出力(git管理外。Releasesへ上げる元)
```

## 最新のインストーラーの置き場 / Where to get the latest

- **Windows**: <https://github.com/aon-co-jp/maid-cafe-se/releases/latest/download/maid-cafe-se-installer.exe>
- **Android**: [Releases](https://github.com/aon-co-jp/maid-cafe-se/releases/latest) の `maid-cafe-se_<version>_android.apk`
- **フォルダとして見る**: [`installer` ブランチ](https://github.com/aon-co-jp/maid-cafe-se/tree/installer)(`windows/`と`android/mobile/`に、最新版だけが入っている)

## 自動で整理する仕組み / Housekeeping

リリースを公開すると、GitHub Actions(`.github/workflows/installer-housekeeping.yml`)が自動で次を行います。

1. 最新リリースの exe と APK だけを、`installer` ブランチに**1コミットで作り直して**強制pushする。古い版はGitから参照されなくなるので、リポジトリの容量は増え続けません(常に最新の1組ぶん)。
2. 古いリリースとそのタグを、新しい順に**3件だけ残して**削除する(件数は、手動実行の`keep`で変えられる)。

ビルドと署名(鍵はリポジトリの外)は、これまでどおり手元で行い、`gh release create`でリリースを公開します。