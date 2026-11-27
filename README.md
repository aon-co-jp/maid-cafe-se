# maid-cafe-se

**maid-cafe-se** — SEは secretary(秘書)の略。「maid-cafe による秘書サービス」。
Androidスマホを、メイドカフェ風(または低い男性の声)の専属音声秘書にするオープンソースアプリ。

> **状況(2026-09-30): v0.5.3 動作版(Androidエミュレータで実機能検証済み、実機端末での検証は未実施)。**
> インストーラー(Windows)・署名つきAPK(Android)を[Releases](https://github.com/aon-co-jp/maid-cafe-se/releases/latest)で配布。入手・インストール方法は[`installer/`](installer/README.md)。

## 主な機能(計画)

1. **定型固定スケジュール**(Googleカレンダー非依存でも動く)
   - 毎日 / 平日 / 土日祝日 / 曜日指定(複数可)
   - 第N週のX曜日(例: 第2火曜)/ N週間ごとのX曜日(例: 2週間ごとの月曜)
   - 祝日は日本の祝日カレンダー(内閣府公開データ)で判定
2. **鳴らし方を選択**
   - **著作権フリーの音**(CC0/パブリックドメインのみ同梱)を目覚まし・アラームとして再生
   - **入力した文章の読み上げ**(端末内TTS)。声は選択制:
     - **メイドカフェ風**(高めの声+「〜ですわ、ご主人様♪」等の口調変換)
     - **太くて低い男性の声**(低ピッチ・ゆっくり)
3. **Googleカレンダー連動**
   - 予定の時刻になったらタイトル等を読み上げ
   - **「30分前に予告」チェックボックス**(予定ごと/全体既定の両方。予告時間は将来的に変更可能にする)

## 実装済み(v0.3.0)

- `core/`(純Kotlin、JVMテスト28件): 繰り返しルール(毎日/平日/土日祝/曜日/第N週/N週ごと)、日本の祝日(2007〜2099年、振替休日・国民の休日・五輪特例対応、外部データ不要)、次発火の計算(`Planner`)、保存形式(`Codec`)
- `app/`(Android、Jetpack Compose): アラーム一覧・追加/編集、音/読み上げ、声の選択(メイド風/低い男性)、30分前予告チェックボックス、Googleカレンダー連動(端末同期分を`CalendarContract`で読み取り、READ_CALENDAR権限)、再起動後の復元、テスト再生
- **メイドのセリフ**(v0.2.0〜): 目覚まし用の「ご主人さま～、お～き～て～。今日も頑張って～」(ゆっくり間延びして読む。**喋る順は番号で決まる**)、「おかえりなさいませご主人様！」「おいしくな～れ萌え萌えキュ～ン」「メッ！ダメなんだぞこら！…」「ファイト！ファイト！」「エクセレント！」「パーフェクト！」をチェックボックスで選択。1つでも複数でも続けて喋る。指定時刻用と、30分前予告用を別々に選べる。アラームとカレンダー連動の両方で使える。
- **喋る順の番号**: 選んだセリフの横に番号が出て、書き換えられる。番号の小さい順に喋る。同じ番号を後から別のセリフに入れると、元の持ち主は空いている最小の番号へ自動で振り直され、**番号はかぶらない**。外すと欠番になり、付け直すと最後の番号の次に加わる。
- **メイドちゃん2人でハモる**(v0.2.0): 同じ音声から長3度違い(平均律4半音)の声を作り、18msずらして重ねる。2つの声を別々に走らせず1つの音声から作るため、タイミングは完全に揃う。
- **音声の後処理**(v0.2.0、`core/audio`): 端末TTSの出力をWAVに書き出し、こちらで加工してから再生する。
  - ピッチと声の太さ(フォルマント)を高品質リサンプラ+WSOLAで変換。エンジンの`setPitch`に頼らない。
  - **make-diskの`resample_poly`(Kaiser窓の多相リサンプラ、scipy互換で精度検証済み)を移植**して使用(折り返し雑音を出さない)。16bit化にはmake-diskと同じ考え方の三角ディザ。
  - 低音の太さ(ローシェルフ)・高音のきらめき(ハイシェルフ)・ハイパス・ピーク正規化・フェード。
  - 合成/解析/加工のどこかで失敗したら、従来の直接読み上げに自動で切り替わる。
- **声の磨き込み**(v0.3.0):
  - セリフごとに1文ずつ合成し、**セリフごとの間(250〜400ms)と抑揚(音程・話速の倍率)**を付けて繋ぐ(例: 「おいしくな～れ萌え萌えキュ～ン」はゆっくり少し高め、「ファイト！」は速め)。
  - 前後の無音トリム、**音量(RMS)の統一**、ソフトリミッター。短い言葉と長い文の聴こえの大きさが揃う。
  - **声の元になる端末の日本語音声を、声質(メイド風/低い男性)ごとに選べる**(メイン画面。未選択は自動)。
- 音源は`tools/gen-sounds`(Rust。旧Python版とバイト単位で同一の出力)で数式生成(外部音源なし、CC0)
- エミュレータ実機検証済み: アラーム発火(Doze考慮の`setAlarmClock`)、TTS読み上げ、音の再生、カレンダー予定の30分前予告、次回の自動再登録

## Windows版(v0.5.0〜: Rust + RPoem製)

`maid-cafe-se-installer.exe`(約1.5MB、Java不要)で入る。v0.4.0まではKotlin(Compose Desktop)製だったが、v0.5.0で**Rustに置き換えた**(`crates/maid-cafe-desktop`)。
アラームの保存形式(`%APPDATA%\maid-cafe-se\alarms.txt`・`settings.properties`)は旧版と同じなので、旧版の設定はそのまま読める。

- 構成: 1つのexe。**RPoem(Poem互換)のローカルWebサーバー**(`127.0.0.1`のみ)が画面(HTML/JS)とJSON APIを出し、スケジューラ・タスクトレイ(`tray-icon`)・Windows標準の日本語音声(SAPI、Microsoft Haruka等)をつなぐ。画面はEdgeのアプリ表示(なければ既定のブラウザ)で開く。
- 画面: アラームの追加/編集/削除/有効切替、繰り返し(毎日・平日・祝日除く平日・土日祝・曜日・第N週・N週ごと)、音/読み上げ、メイド風/低い男性、ハモり、予告(何分前)、**メイドのセリフと喋る順の番号**(重複しない自動振り直しはcoreのルールそのまま)、テスト再生、声の元になるWindows音声の選択、Windows起動時の自動起動。
- 音声の後処理(声の太さの独立制御・ハモり・音量統一)はRPoemの共有クレート`open-runo-voice`(`crates/maid-cafe-core`経由)。合成は区切りごとにSAPI→加工→間を挟んで連結。
- 安全対策: `127.0.0.1`にだけ待ち受け、`Host`/`Origin`の検査(DNSリバインディング対策)、POSTには専用ヘッダ必須(他サイトからの送信を防ぐ)、本文は64KBまで。二重起動は1つにまとめ、2つ目は起動済みの画面を開く。
- コマンドライン: `--minimized`(画面なしでトレイ常駐。自動起動用)、`--no-tray`、`--autostart on|off`、`--selftest [出力先] [--play]`(読み上げの自己診断。`report.txt`にも結果を残す)。
- Googleカレンダー連動はAndroid版のみ(Windows版は未対応)。

## macOS / Linux版(v0.5.6〜、**実機未検証**) / macOS and Linux (untested on real machines)

`crates/maid-cafe-desktop`はWindows以外でもビルドできるようにした(同じコード、OSごとの部分だけ`cfg`で分岐)。インストーラーはGitHub Actions(`.github/workflows/desktop.yml`)が作る。
**開発機はWindowsのため、macOS/Linuxの実機では動かしていない。CIで確認できているのは「ビルド・単体テスト・パッケージ化」まで**(画面表示・トレイ・実際に鳴る音・ログイン時の自動起動は未確認)。

| | Windows | macOS | Linux |
|---|---|---|---|
| 読み上げ | SAPI(PowerShell経由) | `say`(標準搭載、`--file-format=WAVE`でWAV出力) | `espeak-ng`(別途インストール。`espeak`も可) |
| 再生 | `PlaySound` | `afplay` | `paplay`/`pw-play`/`aplay`/`ffplay`のうち最初に使えたもの |
| トレイ | タスクトレイ | メニューバー(Dockには出さない) | AppIndicator(GTK、`libayatana-appindicator3`) |
| 自動起動 | レジストリRunキー | `~/Library/LaunchAgents/jp.co.aon.maid-cafe-se.plist` | `~/.config/autostart/maid-cafe-se.desktop` |
| 保存先 | `%APPDATA%\maid-cafe-se` | `~/Library/Application Support/maid-cafe-se` | `~/.local/share/maid-cafe-se` |
| 画面を開く | Edgeアプリ表示 | `open` | `xdg-open` |
| 配布物 | NSISインストーラー | `.dmg`(universal、ad-hoc署名のみ・未公証) | `.deb`、`.tar.gz`(`install.sh`で`~/.local`へ) |

- macOS: 署名・公証をしていないため、初回は「右クリック→開く」が必要(または`xattr -dr com.apple.quarantine /Applications/maid-cafe-se.app`)。日本語の読み上げは、「システム設定→アクセシビリティ→読み上げコンテンツ」でKyoko等の日本語音声を入れる。
- Linux: 読み上げに`espeak-ng`が必要(`.deb`は自動で入り、`.tar.gz`の`install.sh`も無ければ自動導入する。手動なら`sudo apt install espeak-ng`)。`espeak-ng`の日本語は機械的な声で、Windows/macOSより品質が低い(後段の声の加工は同じ)。`speech-dispatcher`はWAV出力できないため使っていない。AppImage(`maid-cafe-se-<ver>-x86_64.AppImage`、FUSEが必要、`espeak-ng`は同梱せず別途導入)もCIで生成する(実機未検証)。
- 既知の差: Googleカレンダー連動はAndroid版のみ。macOS/Linuxの声の性別は`say`が教えてくれないため、日本語の既知の声(Kyoko等)以外は「不明」として扱う。

The desktop crate now also builds on macOS and Linux (OS-specific parts behind `cfg`). Speech: macOS `say`, Linux `espeak-ng` (both write WAV, then the same DSP/playback pipeline as Windows); playback via `afplay` / `paplay`·`pw-play`·`aplay`·`ffplay`; tray via menu bar / AppIndicator; autostart via LaunchAgent / XDG autostart. Installers (`.dmg`, `.deb`, `.tar.gz`) are built by GitHub Actions on `v*` tags.
**Not verified on real macOS/Linux machines** (the dev machine is Windows): CI only proves build + unit tests + packaging. The macOS app is ad-hoc signed and not notarized.

## 声の抑揚・かわいさ、声でアラーム(v0.5.1)

- **メイドちゃんの声をよりかわいく**: 音程を1.12→1.20倍、声の太さ(声道の小ささ)を1.06→1.10倍、高域のきらめきを+3→+4dB(共有クレート`open-runo-voice`のレシピ)。低い男性の声は変えていない。
- **抑揚**: 端末のTTSは一本調子なので、メイドの声は文(セリフ・本文)を読点・句点・感嘆符で文節に割り、**文節ごとに音程と話速を変えて合成し直す**(上下に揺らし、「！」「？」の語尾は上げてゆっくり、「。」は少し下げる。文節の間は90〜140ms)。`SpeechText::intonate`(Rust)。Windows版・Android版とも、Planner(Rust)経由で同じ処理。
- **声でアラーム(目覚まし)**: 編集画面の「声でアラーム(目覚まし)」にチェックすると、音は鳴らさず声だけで知らせる。チェック時の「音も一緒に鳴らす」をオンにすると、声の下に音を流す(既定はオフ)。保存形式は`ss=1`をオンのときだけ書く(旧版の行と互換)。
- **未検証**: 耳での聴こえ(かわいさ・自然さ)は数値でしか見ていない。文節ごとの再合成でTTSの呼び出しが増える分、合成に時間が掛かる(Android エミュレータで4文節=約0.3秒のRust加工+TTS合成)。
## 10言語の読み上げ、人らしい声(v0.5.2)

- **読み上げの言語**: 編集画面の「読み上げの言語」で、日本語・English・中文(简体)・한국어・Italiano・Français・Deutsch・Русский・فارسی(ペルシャ語)・العربية(アラビア語)から選ぶ。メイドのセリフ7つと、時間になったときの定型文(指定時刻・予告)が、その言語になり、その言語の端末音声で読み上げる。Windows版・Android版とも。
- **翻訳サービスは使わない**: セリフと定型文は、各言語の文を内蔵(`crates/maid-cafe-core/src/lang.rs`)。自由に入力した文章は翻訳せず、入力したまま読む。**内蔵の訳はAI(Claude)が下訳したもので、各言語のネイティブによる確認はしていない。**
- **端末にその言語の音声が必要**: Windowsは「設定→時刻と言語→音声」で追加(ペルシャ語は標準の音声が無いことが多い)。Androidは、端末のTTSにその言語の音声データが必要。無いときは、「その言語の音声が入っていません」と表示して、チャイムに切り替える。
- **メイドの声を人らしく**: ビブラート+ゆっくりしたふらつき+わずかな音量の揺れを付けて、機械的な一定さを崩した(音程±1%以内)。**男性の声には付けていない**(すでに人らしいとの指摘のため)。ハモりの2人目は、声の太さを1人目よりはっきり細くし、揺れの速さも変え、ずれを18→32msにして、別の人らしくした。
- 未検証: 聴こえ方(人らしさ・かわいさ・各言語の自然さ)。Windowsでの英語以外の音声、Androidの英語以外の言語の実機確認。
## ビルド

```
cargo test --workspace                  # Rust: core(祝日・繰り返し・セリフ番号・保存形式)、Windows版(API・スケジューラ・HTTP結合)
cargo clippy --workspace --all-targets -- -D warnings
gradlew :core:test :app:assembleDebug   # Android: Kotlin(画面・OS連携)+ Rust製コア(JNI)。:core:testはホスト向けにビルドしたRustのDLLを本物のJNIで読み込む
```

Rustのビルドには、兄弟ディレクトリに`RPoem`(`F:\RPoem`)が必要(パス依存。`open-english`などと同じ)。Windowsインストーラーは`installer\build-windows.ps1`。

## 既知の制約

- 声のもとになるのは端末のTTSエンジンの音声。後処理(ピッチ/声の太さ/EQ)で「メイド風」「太く低い男性」に寄せているが、元の声質・自然さは端末のTTSに依存する。**主観的な音質(自然さ)は開発機のエミュレータで耳では確認できておらず、数値(音程比・長さ・ピーク)とテストでの検証のみ**。男性/女性声の選択は音声名からの推定(best-effort)。
- カレンダーは終日予定を読み上げ対象外。先7日分を取り込み、毎日4時と起動時に再取得。
- 祝日は2007年以降のみ対応。
- 実機端末(省電力設定・メーカー独自のバッテリー制限)での長時間検証は未実施。

## 技術方針

| 項目 | 方針 |
|---|---|
| 言語/UI | Kotlin + Jetpack Compose |
| 時刻起動 | `AlarmManager.setAlarmClock()`(Doze中も確実、`USE_EXACT_ALARM`/`SCHEDULE_EXACT_ALARM`を検討) |
| 再起動後の復元 | `BOOT_COMPLETED`受信で再登録 |
| 読み上げ | Android `TextToSpeech`(端末内、オフライン可)。声色はピッチ/速度+端末の男性/女性声から選択 |
| カレンダー | 端末に同期済みのGoogleカレンダーを`CalendarContract`経由で読み取り(OAuth不要、外部送信なし)。採用済み |
| 繰り返し判定 | RFC 5545 RRULE準拠の内部エンジン(第N週・N週間ごと対応)+祝日データ |
| 音源 | CC0のみ。出典・ライセンスを`assets/sounds/LICENSES.md`に必ず記録 |
| ライセンス | 未決定(OSS、採用時に明記) |

## プライバシー方針

- カレンダー内容は端末内で処理し、外部サーバーへ送信しない。
- アカウント情報・トークンをリポジトリへ含めない。

## 次回再開ポイント

[`PORTING.md`](PORTING.md)を参照。

## 関連

- エコシステム索引: [aon-co-jp/runo](https://github.com/aon-co-jp/runo)

---

作者 / Author: 石 塚 正 浩（MASAHIRO ISHIZUKA） (https://github.com/aon-co-jp)

## maid-cafe.tokyo 設計書 / Design document for maid-cafe.tokyo

目覚ましアプリとは別件の、学習サイト maid-cafe.tokyo(メイドカフェ風の先生による英会話・開発・学校の勉強・研修、出典つきニュースとデータ分析)の設計書を [`docs/maid-cafe-tokyo-design.md`](docs/maid-cafe-tokyo-design.md) に置いています(設計のみ・コード未着手)。再開手順は [`PORTING.md`](PORTING.md)。

A separate-track design document for the maid-cafe.tokyo learning site (maid-café-style tutor for English, development, school subjects and training, plus sourced news and data analysis) lives in [`docs/maid-cafe-tokyo-design.md`](docs/maid-cafe-tokyo-design.md). Design only; no code yet. Resume steps are in [`PORTING.md`](PORTING.md).

## 配布形式の追加 (v0.5.6以降) / Additional distribution formats

- **Android `.aab`**: `installer\build-aab.ps1`(JDK17、鍵情報は`build-release.ps1`と同じ外部ファイル)で署名つき`maid-cafe-se_<ver>_android.aab`を`installer\dist`へ出す。ローカルでjarsigner検証済み(自己署名のため警告は出る)。**Google Playへのアップロードは未検証**。`SHA256SUMS.txt`にはAPKの行を残したまま.aabの行を足す。
- **Linux AppImage**: CI(`desktop.yml`)で`installer/build-linux.sh`が生成する。**実機で起動は未検証**。
- **macOS署名・公証**: GitHub Secretsが全て設定された時だけ Developer ID署名+`notarytool`公証を行う(無ければ従来どおり未署名の.dmg)。**Apple Developerアカウントが無いため、実署名・公証は一度も動かしていない(未検証)**。Secrets名と手順はPORTING.md。
- **Android `.aab`**: `installer\build-aab.ps1` builds a signed bundle into `installer\dist` (verified with jarsigner locally; Play upload not tested). **Linux AppImage** is built in CI (not run on real machines). **macOS Developer ID signing + notarization** runs only when all GitHub Secrets are set, otherwise the unsigned .dmg as before; **never executed (no Apple Developer account) — unverified**.

## API 方針: REST 全廃・gRPC / Connect へ (2026-11-04)

### 日本語
- **REST は全廃する。新しい API は REST で作らない。** ブラウザや `curl` から呼ぶ API は **Connect の単純呼び出し**(`POST /<パッケージ>.<Service>/<Method>`、`content-type: application/json`、本文は JSON)、サービス間は **gRPC / gRPC-web**(RPoem の管理API `open_runo.admin.v1.Admin` と同じ流儀)。ストリーミングは、必要になってから。
- **既存の REST は、触るときに順に移す。** 移したら旧 REST の経路は消す。移行が終わるまで残るが、**新規には追加しない**。
- **JSON は RS-JSON に統一**する(読み込みは `from_str_secure` / `parse_secure` でサイズ・深さ・重複キーの上限つき、書き出しは `rust_json::fast`)。ブラウザの WASM に `prost` は入れない(サイズが増えるため)。
- **例外(自分たちの API ではなく、相手の規格が決めているもの)**: ACME、OAuth/OIDC のコールバック、第三者(GitHub など)からの Webhook、ヘルスチェック、静的ファイル配信、OTLP など。これらは規格どおりの HTTP を受ける。
- 参考実装: `RPoem`(`open_runo.admin.v1.Admin`)、`open-easy-web`(`server/src/connect.rs`、`proto/open_easy_web/admin/v1/admin.proto`)。
- ※ このリポジトリのコードが既に方針どおりかどうかは、各リポジトリの現状による。この節は方針の記録であり、コードの変更を意味しない。

### English
- **REST is being abolished. New APIs are not built as REST.** APIs called from browsers or `curl` use **Connect unary calls** (`POST /<package>.<Service>/<Method>`, `content-type: application/json`, JSON body); service-to-service calls use **gRPC / gRPC-web** (the same style as RPoem's admin API `open_runo.admin.v1.Admin`). Streaming only when it is actually needed.
- **Existing REST endpoints are migrated one by one, when they are touched.** The old REST route is removed once migrated. Until migration is finished they remain, but **no new ones are added**.
- **JSON is unified on RS-JSON**: reading with `from_str_secure` / `parse_secure` (size, depth and duplicate-key limits), writing with `rust_json::fast`. `prost` is not put into browser WASM (it would grow the binary).
- **Exceptions (shapes dictated by someone else's standard, not our own API)**: ACME, OAuth/OIDC callbacks, webhooks from third parties (e.g. GitHub), health checks, static file serving, OTLP, and so on. These accept plain HTTP as the standard requires.
- Reference implementations: `RPoem` (`open_runo.admin.v1.Admin`) and `open-easy-web` (`server/src/connect.rs`, `proto/open_easy_web/admin/v1/admin.proto`).
- Note: whether the code in a given repository already follows this policy depends on that repository. This section records the policy; it does not mean the code has been changed.

### Deutsch (KI-Übersetzung, redaktionell überarbeitet; noch nicht von Muttersprachlern geprüft)
- **REST wird vollständig abgeschafft; neue APIs entstehen nicht mehr als REST.** APIs, die aus dem Browser oder per `curl` aufgerufen werden, verwenden **unäre Connect-Aufrufe** (`POST /<Paket>.<Service>/<Methode>`, `content-type: application/json`, JSON-Body). Zwischen Diensten kommen **gRPC bzw. gRPC-web** zum Einsatz, im selben Stil wie die Admin-API von RPoem (`open_runo.admin.v1.Admin`). Streaming wird erst ergänzt, wenn es tatsächlich gebraucht wird.
- **Bestehende REST-Endpunkte werden schrittweise umgestellt, und zwar immer dann, wenn man ohnehin an ihnen arbeitet.** Sobald ein Endpunkt umgestellt ist, wird die alte REST-Route entfernt. Bis zum Abschluss der Migration bleiben die übrigen bestehen, es werden aber **keine neuen mehr hinzugefügt**.
- **JSON wird einheitlich über RS-JSON verarbeitet**: Gelesen wird mit `from_str_secure` bzw. `parse_secure` (mit Obergrenzen für Größe, Verschachtelungstiefe und doppelte Schlüssel), geschrieben mit `rust_json::fast`. `prost` wird nicht in das Browser-WASM aufgenommen, da es die Datei vergrößern würde.
- **Ausnahmen (Schnittstellen, deren Form ein fremder Standard vorgibt und die nicht zu unserer eigenen API gehören)**: ACME, OAuth/OIDC-Callbacks, Webhooks von Drittanbietern (z. B. GitHub), Health-Checks, die Auslieferung statischer Dateien, OTLP und Ähnliches. Diese nehmen, wie vom jeweiligen Standard verlangt, gewöhnliches HTTP entgegen.
- Referenzimplementierungen: `RPoem` (`open_runo.admin.v1.Admin`) und `open-easy-web` (`server/src/connect.rs`, `proto/open_easy_web/admin/v1/admin.proto`).
- Hinweis: Ob der Code eines einzelnen Repositorys diese Richtlinie bereits erfüllt, hängt vom jeweiligen Repository ab. Dieser Abschnitt dokumentiert lediglich die Richtlinie; er besagt nicht, dass der Code schon angepasst wurde.

### Русский (перевод выполнен с помощью ИИ и отредактирован; носителями языка пока не проверен)
- **От REST мы полностью отказываемся; новые API больше не создаются в стиле REST.** API, к которым обращаются из браузера или через `curl`, используют **унарные вызовы Connect** (`POST /<пакет>.<Service>/<Method>`, `content-type: application/json`, тело запроса — JSON). Для взаимодействия между сервисами применяются **gRPC и gRPC-web** — в том же стиле, что и административный API RPoem (`open_runo.admin.v1.Admin`). Потоковая передача данных добавляется лишь тогда, когда в ней действительно возникает необходимость.
- **Существующие REST-эндпоинты переводятся поэтапно — по мере того, как с ними приходится работать.** После перевода старый REST-маршрут удаляется. Пока миграция не завершена, оставшиеся эндпоинты сохраняются, но **новые не добавляются**.
- **Работа с JSON унифицирована на основе RS-JSON**: чтение — через `from_str_secure` или `parse_secure` (с ограничениями на размер, глубину вложенности и повторяющиеся ключи), запись — через `rust_json::fast`. `prost` в браузерный WASM не включается, поскольку это увеличило бы размер файла.
- **Исключения (интерфейсы, форму которых определяет чужой стандарт, а не наш собственный API)**: ACME, обратные вызовы OAuth/OIDC, вебхуки сторонних сервисов (например, GitHub), проверки работоспособности, раздача статических файлов, OTLP и тому подобное. Такие интерфейсы принимают обычный HTTP, как того требует соответствующий стандарт.
- Эталонные реализации: `RPoem` (`open_runo.admin.v1.Admin`) и `open-easy-web` (`server/src/connect.rs`, `proto/open_easy_web/admin/v1/admin.proto`).
- Примечание: соответствует ли код конкретного репозитория этой политике, зависит от самого репозитория. Данный раздел лишь фиксирует политику и не означает, что код уже изменён.

## API 方針の補足: 用語の整理と使い分け (2026-11-04)

### 日本語
**1つに統一するものではなく、別の層の話です。** 「REST の代わり」になるのは Connect・gRPC・GraphQL だけで、WASM は画面を作る技術です。

| 用語 | 何か | REST の代わりか |
|---|---|---|
| REST | URL と HTTP メソッドで操作を表す API の作り方 | 全廃する側 |
| **Connect** | `POST /サービス名/メソッド名` に JSON を送る形式。`curl` でもブラウザの `fetch` でも呼べる | はい(**採用**) |
| **gRPC** | 同じ「サービス/メソッド」の考え方を、バイナリ(protobuf)と HTTP/2 で行う形式。速く、ストリーミングも得意 | はい(**サービス同士に採用**) |
| gRPC-web | gRPC のブラウザ向けの変形版 | はい(必要なら) |
| GraphQL | 欲しい項目を問い合わせる形式 | 一部だけ(**問い合わせ(読み取り)専用**) |
| WASM | Rust をブラウザの中で動かす仕組み | いいえ(画面を作る技術) |

- **ブラウザの画面や `curl` から呼ぶ API は Connect**(POST、JSON を RS-JSON で読み書き)。ブラウザの WASM は素の gRPC を話せず、Connect なら `fetch` のままで、`prost` を足さずに済む(画面のサイズも増えない)。
- **サービス同士の通信(サーバー間、重い処理、ストリーミング)は gRPC。** 多くは Connect で足りるので、ストリーミングが必要になった所だけ gRPC に上げる。
- **GraphQL は、まとめて問い合わせたいときの読み取り専用。** 書き込み(操作)には使わない。
- **WASM は画面を作る技術として、そのまま使う。** 画面から呼ぶ API が Connect になっただけ。
- 外部の規格が形を決めるもの(ACME、OAuth のコールバック、第三者の Webhook、ヘルスチェック、静的ファイル、OTLP)は、規格どおりの HTTP のまま。
- **迷ったら Connect。**

### English
**These are not alternatives to unify; they sit at different layers.** Only Connect, gRPC and GraphQL can replace REST; WASM is a way to build the UI.

| Term | What it is | Replaces REST? |
|---|---|---|
| REST | API style that expresses operations through URLs and HTTP methods | The thing being abolished |
| **Connect** | `POST /Service/Method` with a JSON body; callable from `curl` and from a browser `fetch` | Yes (**adopted**) |
| **gRPC** | The same service/method idea over binary (protobuf) and HTTP/2; fast, good at streaming | Yes (**for service-to-service**) |
| gRPC-web | A browser-friendly variant of gRPC | Yes (if needed) |
| GraphQL | A query language: ask for exactly the fields you want | Partly (**read-only queries**) |
| WASM | Runs Rust inside the browser | No (a UI technology) |

- **APIs called from browser screens or `curl` use Connect** (POST, JSON read and written with RS-JSON). Browser WASM cannot speak plain gRPC; with Connect it stays a `fetch`, and `prost` is not needed (the binary does not grow).
- **Service-to-service calls (server to server, heavy work, streaming) use gRPC.** Connect is enough in most places; move a call to gRPC only where streaming is actually needed.
- **GraphQL is read-only**, for queries that gather several pieces of data. It is not used for writes (operations).
- **WASM stays as the way to build the UI.** The API it calls simply became Connect.
- Interfaces whose shape an outside standard dictates (ACME, OAuth callbacks, third-party webhooks, health checks, static files, OTLP) stay plain HTTP.
- **When in doubt, use Connect.**

### Deutsch (KI-Übersetzung, sprachlich überarbeitet; noch nicht von Muttersprachlern geprüft)
**Es geht nicht darum, eine einzige Technik auszuwählen – die Begriffe liegen auf unterschiedlichen Ebenen.** Als Ersatz für REST kommen nur Connect, gRPC und GraphQL in Frage; WASM dagegen ist eine Technik, mit der die Benutzeroberfläche gebaut wird.

| Begriff | Was ist das? | Ersatz für REST? |
|---|---|---|
| REST | Ein API-Stil, bei dem Operationen über URLs und HTTP-Methoden ausgedrückt werden | wird abgeschafft |
| **Connect** | `POST /Dienst/Methode` mit JSON im Body; aufrufbar mit `curl` ebenso wie mit `fetch` im Browser | ja (**gewählt**) |
| **gRPC** | Dasselbe Prinzip aus Dienst und Methode, aber über binäres protobuf und HTTP/2; schnell und gut für Streaming geeignet | ja (**zwischen Diensten**) |
| gRPC-web | Eine für den Browser angepasste Variante von gRPC | ja (bei Bedarf) |
| GraphQL | Abfragesprache, mit der man genau die benötigten Felder anfordert | teilweise (**nur für lesende Abfragen**) |
| WASM | Führt Rust-Code direkt im Browser aus | nein (Technik für die Oberfläche) |

- **APIs, die von Browseroberflächen oder per `curl` aufgerufen werden, verwenden Connect** (POST; JSON wird mit RS-JSON gelesen und geschrieben). WASM im Browser kann kein reines gRPC sprechen. Mit Connect bleibt es bei einem gewöhnlichen `fetch`, und `prost` ist nicht nötig – die WASM-Datei wird dadurch nicht größer.
- **Für die Kommunikation zwischen Diensten (Server zu Server, rechenintensive Aufgaben, Streaming) wird gRPC eingesetzt.** In den meisten Fällen reicht Connect aus; auf gRPC wird nur dort umgestellt, wo Streaming tatsächlich gebraucht wird.
- **GraphQL dient ausschließlich lesenden Abfragen**, wenn mehrere Informationen gebündelt abgefragt werden sollen. Für Schreibzugriffe (Operationen) wird es nicht verwendet.
- **WASM bleibt die Technik für die Oberfläche.** Geändert hat sich lediglich, dass die aufgerufene API nun auf Connect beruht.
- Schnittstellen, deren Form durch einen externen Standard festgelegt ist (ACME, OAuth-Callbacks, Webhooks von Drittanbietern, Health-Checks, statische Dateien, OTLP), bleiben gewöhnliches HTTP.
- **Im Zweifel: Connect.**

### Русский (перевод выполнен с помощью ИИ и литературно отредактирован; носителями языка пока не проверен)
**Это не варианты, из которых нужно выбрать один, а технологии разных уровней.** Заменить REST могут только Connect, gRPC и GraphQL; WASM же — это способ создания пользовательского интерфейса.

| Термин | Что это | Заменяет REST? |
|---|---|---|
| REST | Стиль построения API, при котором операции выражаются через URL и HTTP-методы | отменяется |
| **Connect** | `POST /Сервис/Метод` с телом в формате JSON; вызывается как из `curl`, так и из браузерного `fetch` | да (**выбран**) |
| **gRPC** | Тот же принцип «сервис — метод», но поверх двоичного формата protobuf и HTTP/2; работает быстро и хорошо подходит для потоковой передачи | да (**для связи между сервисами**) |
| gRPC-web | Адаптированный для браузера вариант gRPC | да (при необходимости) |
| GraphQL | Язык запросов: клиент запрашивает ровно те поля, которые ему нужны | частично (**только запросы на чтение**) |
| WASM | Позволяет выполнять код на Rust прямо в браузере | нет (технология для интерфейса) |

- **API, которые вызываются из браузерных интерфейсов или через `curl`, строятся на Connect** (POST; JSON читается и записывается через RS-JSON). Браузерный WASM не умеет работать с «чистым» gRPC. С Connect достаточно обычного `fetch`, а `prost` не требуется — размер WASM-файла не увеличивается.
- **Для обмена между сервисами (сервер — сервер, ресурсоёмкие задачи, потоковая передача) применяется gRPC.** В большинстве случаев достаточно Connect; на gRPC переходят только там, где потоковая передача действительно необходима.
- **GraphQL используется только для чтения**, когда нужно получить сразу несколько связанных данных. Для записи (операций) он не применяется.
- **WASM по-прежнему остаётся технологией для интерфейса.** Изменилось лишь то, что вызываемый им API теперь построен на Connect.
- Интерфейсы, форма которых задана внешним стандартом (ACME, обратные вызовы OAuth, вебхуки сторонних сервисов, проверки работоспособности, статические файлы, OTLP), остаются обычным HTTP.
- **Если есть сомнения — выбирайте Connect.**

### 検証結果 (2026-11-13) / Verification results

- 検証済み: v0.5.7のリリース添付物のSHA-256がSHA256SUMSと一致。.aabは`jarsigner -verify`で署名OK、.aabとAPKの署名証明書は同一(`CN=maid-cafe-se`)。WSL2(Ubuntu 24.04、実機のLinuxではない)で、AppImageと`.deb`(`apt install`で依存も解決)の`--selftest`が通り、日本語の合成が動いた。
- 分かったこと: AppImageはライブラリを同梱しない。`libgtk-3-0`・`libxdo3`・`libayatana-appindicator3-1`・`espeak-ng`が無いと`libxdo.so.3`が無いと言って起動しない(`.deb`はDependsで自動導入)。FUSEの有無とは別の話。
- 未検証のまま: macOS全般(署名・公証・.dmgの実機動作)、Google Playへの.aabアップロード、Linuxのトレイ/画面/自動起動(WSLにはデスクトップが無い)、実機のLinux。
- Verified: release asset hashes match; .aab passes `jarsigner -verify` and shares the APK's signing cert; AppImage and `.deb` pass `--selftest` under WSL2 (Ubuntu 24.04, not a real Linux desktop). Found: the AppImage bundles no libraries and needs libgtk-3-0, libxdo3, libayatana-appindicator3-1 and espeak-ng installed. Still unverified: everything on macOS, Play upload of the .aab, Linux tray/GUI/autostart.

### 2026-11-17 追記: v0.5.7の再検証と再開用メッセージ / Re-verification and resume message

**日本語**
- 確認済み: 添付7ファイルのSHA-256が`SHA256SUMS*.txt`と一致。APK(署名方式v2)と`.aab`は同一の署名証明書(`CN=maid-cafe-se, O=aon-co-jp`、SHA-256 `104cd63b…ea12af`、自己署名)。AppImageをWSL(Ubuntu)で起動し、ローカルWeb UI(127.0.0.1)がHTTP 200。リリース本文は、添付一覧・確認済み・未検証の3区分に書き換え済み。
- 未検証のまま: Macの署名・公証(リポジトリにAppleのシークレットが未登録で、CIの署名処理は一度も実行されていない)、macOS実機、実デスクトップのLinux(トレイ・音声)、Windowsインストーラーの通し試験、.aabのGoogle Playアップロード。
- 落とし穴: `.github/workflows/desktop.yml`(125行目付近)が、新しいタグごとにリリース本文を旧い自動生成文(「未署名・実機での動作は未確認」)で作る。次のタグを打つ前に、同じ3区分の文へ直す。`Cargo.lock`に未コミットの差分があるが本件とは無関係(盲目的にコミットしない)。
- **再開用メッセージ(別アカウントへ貼り付け)**: 「`F:\maid-cafe-se`(aon-co-jp/maid-cafe-se)と`F:\runo`で再開。v0.5.7のリリース本文は更新済み。残り: (1)`desktop.yml`のリリース本文の自動生成文を直す、(2)Mac署名は`APPLE_ID`・`APPLE_TEAM_ID`・`APPLE_APP_PASSWORD`をリポジトリのシークレットに登録(値は本人が`F:\`直下のtxt経由で渡す。画面に出さない)してからタグを打って検証、(3)実Linuxデスクトップ・Windowsインストーラー・Play uploadの確認。チャットは日本語のみ。」

**English**
- Verified: the SHA-256 of all 7 attached files matches `SHA256SUMS*.txt`. The APK (signature scheme v2) and the `.aab` share one self-signed certificate (`CN=maid-cafe-se, O=aon-co-jp`, SHA-256 `104cd63b…ea12af`). The AppImage launched under WSL (Ubuntu) and its local web UI (127.0.0.1) returned HTTP 200. The release notes were rewritten into attachments / verified / unverified sections.
- Still unverified: Mac signing and notarization (no Apple secrets are registered in the repository, so the CI signing step has never run), real macOS, a real Linux desktop (tray, voice), an end-to-end Windows installer test, and a Google Play upload of the `.aab`.
- Pitfalls: `.github/workflows/desktop.yml` (around line 125) creates the release notes with the old auto-generated text ("unsigned, not verified on real devices") for every new tag; fix it to the same three-part text before tagging again. `Cargo.lock` has an uncommitted diff unrelated to this work (do not commit it blindly).
- **Resume message (paste into the other account)**: "Resume in `F:\maid-cafe-se` (aon-co-jp/maid-cafe-se) and `F:\runo`. The v0.5.7 release notes are updated. Open: (1) fix the auto-generated release-notes text in `desktop.yml`; (2) for Mac signing, register `APPLE_ID`, `APPLE_TEAM_ID` and `APPLE_APP_PASSWORD` as repository secrets (the owner supplies them via a txt file at the root of `F:\`; never print them), then tag and verify; (3) check a real Linux desktop, the Windows installer and the Play upload. Chat in Japanese only."

## Versionless API の方針 / Versionless API policy (2026-11-25)
**日本語**: このリポジトリのAPIは **Versionless**(URL・パッケージ名に `v1` 等を付けず、互換性を壊さず足すだけで進化させる)。欄は足すだけ・足す欄は省略可能・知らない欄は無視・名前変更は新名を足して旧を非推奨に(期限は既定で6か月以上、使用0を確認してから削除)・enumは未知の値を許容・安全上の緊急修正は例外。既存の `.v1` は呼び出し側を壊さないよう当面残し、版なしの別名を追加して段階的に畳む。詳細・出典は [`runo/API-POLICY.md`](https://github.com/aon-co-jp/runo/blob/main/API-POLICY.md) 末尾。適用はこれから順に行う。
**English**: This repository's APIs are **Versionless**: no `v1` in URLs or package names; evolve by adding only, without breaking compatibility. Only add fields, new fields are optional, ignore unknown fields, rename by adding the new name and deprecating the old (default deadline 6 months or more, remove only after confirming zero use), tolerate unknown enum values, emergency security fixes are the exception. Existing `.v1` names stay for now so callers do not break; a versionless alias is added and `.v1` is retired in stages. Details and sources: the end of [`runo/API-POLICY.md`](https://github.com/aon-co-jp/runo/blob/main/API-POLICY.md). Rollout is step by step.

## 完成まで作り切る方針 / Finish everything policy (2026-11-27)
**日本語**: 全リポジトリで、**未確認は自動で確認し、未実施は実施し、未了は完成させる**(確認を求めず進める)。使用頻度が低いと判断される機能や、**1から作り直す大規模開発になる内容でも、やめずに最後まで開発・TEST・DEBUGする**(別の方式で済ませない、後回しにしない)。進め方: 調査(英語・日本語の検索とGitHub) → 設計 → 実装 → 実機/実データで確認 → 失敗は原因を直して再試験 → README・CLAUDE・PORTING・POLICY を日英で更新 → push。「できた」と書くのは実測した範囲だけ、確認できなかった所は「未確認」と正直に書く。例外(確認を取る): 元に戻せない削除、秘密の公開、所有者の認証・決済・実機操作が要るもの、利用者のシステム設定の変更。
**English**: In every repository, **verify what is unverified, do what is not yet done, and finish what is unfinished** automatically, without asking. Even features that look rarely used, or work that means **a large rewrite from scratch, are developed, tested and debugged to the end** (no substituting another approach, no postponing). Method: research (English and Japanese search plus GitHub) -> design -> implement -> verify on real devices or data -> fix causes and retest on failure -> update README, CLAUDE, PORTING and POLICY in Japanese and English -> push. Write "done" only for what was measured, and say "unverified" honestly for the rest. Exceptions that still need confirmation: irreversible deletions, publishing secrets, anything needing the owner's authentication, payment or real-device actions, and changes to the user's system settings.