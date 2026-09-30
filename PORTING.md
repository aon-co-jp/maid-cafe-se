# PORTING / 開発メモ (maid-cafe-se)

## 現状 (2026-09-30)
- **v0.4.0**: Windows版(`desktop/`、Compose Desktop、`core`と`shared-ui/`をAndroid版と共有)、インストーラー(`installer/`、NSIS→`maid-cafe-se-installer.exe`)、署名つきAPK。Windows版はWindows標準の日本語音声(SAPI)をPowerShell経由で使い、同じVoiceDsp処理を通す。実機(このPC)で、自己診断・スケジューラ発火(狂い2ms)・インストール/上書き/アンインストール/自動起動を確認。desktopテスト3件。
- 音源生成は`tools/gen-sounds`(Rust)へ移行済み(旧Python版と出力がバイト単位で同一)。リポジトリ内のPythonは無くなった。
- **次の大きな課題: アプリ本体のRust(+RPoem)化**(ユーザー指示 2026-09-30)。下の「Rust移行計画」を参照。
- (v0.3.0の後) 喋る順の番号(編集可・重複は自動で振り直し)。`MaidPhrases.assign`が正本。**Setの等価判定は順序を無視する**ため、並べ替えだけの更新はComposeの状態が「変更なし」と見なして再描画されない罠があった(`neverEqualPolicy`と、一覧の更新を削除+挿入にして回避)。core 75テスト、エミュレータで入れ替え・自動振り直し・保存順を確認。
- v0.3.0: 声の磨き込み(セリフごとの間・抑揚、無音トリム、RMS音量統一+ソフトリミッター、端末音声の選択UI)。core 66テスト。エミュレータで5区切りの連結を確認(最終WAVの長さが音声+間の計算値と一致、RMS0.17、ピーク0.90)。エミュレータのjaはhtm/jab(女性)・jac/jad(男性)の4音声で、音声名からの性別推定が一致。
- v0.2.0: メイドのセリフ6種(指定時刻/予告別)・2人ハモり・TTS後処理(VoiceDsp、make-diskのresample_poly移植)。core 57テスト。エミュレータで3声(メイド/ハモり/低音)がDSP経路で再生されることをログとWAV解析で確認(男性声0.84倍・長さ保持・ピーク0.9)。
- v0.1.0: core(28テスト) + Androidアプリ(エミュレータ Pixel_9_Pro / Android 16系で発火・TTS・音・カレンダー予告を実機能検証)。
- ビルド: `JAVA_HOME`にAndroid Studio同梱JBR、`local.properties`のsdk.dirはスラッシュ区切りで書く(バックスラッシュだと"Invalid file path")。
- 検証tips: `adb shell run-as`で`shared_prefs/maidcafese.xml`にアラームを直接仕込める(Git Bashでは`MSYS_NO_PATHCONV=1`必須)。

## 次回再開ポイント(優先順)
0. **ニューラルTTS(声質の根本改善)**: 端末TTSに頼らずオンデバイスで日本語ニューラルTTS(例: sherpa-onnx+VITS系)を使う案。またはmake-diskで検証済みのtract+ONNX(LavaSR帯域拡張、HT-Demucs等)で音声の帯域拡張。いずれもモデル(数十MB〜)のダウンロードとライセンス確認が必要なので、ユーザー許可を得てから。
1. 実機端末での長時間検証(Doze・メーカー独自バッテリー制限・再起動後の復元)
2. 予告分数を変更可能に(現在は30分固定、コア側は任意分対応済み)
3. 男性/女性TTS音声の選択精度向上(端末の音声一覧から選ぶUI)
4. 予定ごとの個別ON/OFF、終日予定の朝アナウンス
5. リリース署名・Play配布の検討、アイコン

## ニューラルTTS調査記録 (2026-09-30、ライセンス確認済み・ダウンロード/組み込みは未実施)

ユーザー許可のもと、端末TTSに代わる日本語ニューラルTTSを調査した。**結論: 配布アプリに同梱して安全と言い切れるモデルは現時点で見つからず、組み込みは保留。**

| 候補 | ライセンス/来歴 | 判定 |
|---|---|---|
| sherpa-onnx公式の事前学習モデル | ランタイムはApache-2.0。ただし公式配布644件に**日本語TTSモデルは無い**(Kokoro multi-lang v1_0/v1_1もsherpa-onnx側は中英のみ対応) | 日本語不可 |
| piper-plus (ayutaz) `piper-plus-base` / `piper-plus-tsukuyomi-chan` | コードはMIT、Android用G2P(OpenJTalk)もMaven Centralにあり技術的には最適。**しかし日本語の学習データがMOE-Speech(ゲーム音声。日本法30条の4の機械学習解析目的に限り許可、再配布禁止、出典ゲーム/声優は秘匿)**。baseモデルはCC-BY-4.0表記だが来歴が不透明で、tsukuyomi-chan版もbaseからのfine-tune | **配布不可(権利リスク)** |
| つくよみちゃんコーパス自体 | 商用可・クレジット必須・合成音声の素材としての二次利用許可は不可(鑑賞用配布は可) | 規約は許容範囲だが、現存モデルがbase由来 |
| Kokoro-82M (hexgrad) | 重みApache-2.0。日本語話者は5人(jf_alpha=C+、他はC/C-評価、koniwa由来CC BYの朗読)。学習に「大手TTSの合成音声」を含むと明記。ONNX(q8f16で約86MB)あり | ライセンスは概ね可だが**日本語品質が低評価**。G2P(misaki[ja]/pyopenjtalk)のAndroid移植が大工事 |
| Style-BERT-VITS2 JP-Extra系 | コードAGPL-3.0(組み込むとアプリ全体のソース公開義務)、重いBERT(数百MB)が必要 | 端末実行に不向き |

次の一手の候補: (a) 出典が明確なコーパス(JSUT/JVS等、CC BY-SA 4.0でshare-alike条件の解釈要確認)から自前でVITS/Matchaを学習(GPU要)、(b) Kokoroの日本語を評価して許容できるか耳で判断、(c) 端末TTS+現行DSP後処理を当面の実用解として磨く→**v0.3.0で実施済み**。

## 未決事項
- 「メイドカフェ風」の口調変換ルールの拡充範囲
- ライセンス

## Rust移行の進捗 (2026-09-30)

- **第1段階(完了): `crates/maid-cafe-core`** — 繰り返しルール・日本の祝日・Planner・Codec(旧Kotlin版と保存形式が互換、Javaの`URLEncoder`と同じ符号化)・メイドのセリフ(喋る順の番号)・音声処理(Kaiser窓リサンプラ・WSOLA・シェルフEQ・ハモり・音量統一)を移植。Kotlinのテストを仕様として移植し、`cargo test -p maid-cafe-core`で77テスト。
- **ゴールデンテスト(`tests/golden.rs`)**: Kotlin版で実際に音声処理を実行した出力(`tests/golden/*.f32`)とRust版の出力を照合し、4パターンすべて**最大誤差0.00000**(同じ入力で同じ音)。
- 実測(同一マシン、4秒の音声、ウォーム最速): Kotlin 単独36ms/ハモり126ms、Rust 単独34ms/ハモり68ms。**Rust化そのものは音質を変えない**(出力が同一)。速度もウォーム時はほぼ同じで、効くのはコールドスタート(JITの立ち上がり無し)・実行環境同梱が不要になること(インストーラーが小さくなる)・WindowsとAndroidで1つの実装を共有できること。
- 音質を実際に上げるには、Rust化とは別に: (1)ピッチとフォルマントを独立に制御する方式(現行はリサンプリングで両方同時に動かす)、(2)AIによる帯域拡張(make-diskで検証済みのtract+ONNX。モデルのダウンロードとライセンス確認が必要)、(3)日本語ニューラルTTS(調査記録の通り、現状は安全なモデルが無い)。

## 音質向上の研究 (2026-09-30)

| 項目 | 内容 | 結果 |
|---|---|---|
| ① 音程と声の太さを独立に制御 | `crates/maid-cafe-core/src/audio/formant.rs`(FFT+ケプストラム包絡の周波数伸縮)、レシピに`formant_ratio` | 直接合成した正解の母音との包絡距離: 新1.6dB vs 旧10.3dB(音程0.72倍・声の太さ据え置き)、独立制御は0.6〜1.1dB(未処理は約8dB)。ゲイン上限は±12dBだと鋭いフォルマントを動かせず±24dBで解決 |
| ② AI帯域拡張(LavaSR) | `crates/maid-cafe-enhance`。make-diskの`audio_sr.rs`の設計を移植(入力の帯域は変えず、高域だけを頭打ちつきで足す)。**声向けのロールオフ検出**を新設(TTSの声は「崖」でなくなだらかに減衰するため、音楽向けの崖検出は12.4kHzを返し無意味だった)。モデルは固定リビジョンから取得しSHA-256検証(同梱も可、`load_bundled_or_download`) | 実データ(Windows音声Haruka)で、ロールオフ7.5kHzを検出。自己教師あり評価(6kHzで帯域制限→拡張→元の音声とのLSD、6〜9.5kHz): 46.8dB→12.9dB、入力の帯域は変化なし(低域LSDが完全一致)。**LSDはスペクトル包絡の近さで聴感品質ではない。拡張前は帯域が無音のため、差の大部分は「何か入れれば縮む」分**。モデル読み込み0.2秒(取得済み時)、処理は音声6.8秒に1.0秒 |
| ③ 日本語ニューラルTTS | 上の「ニューラルTTS調査記録」参照 | 安全に同梱できるモデルは未発見 |

- ライセンス: LavaSR本体・audiosronnxともApache-2.0、学習データはVCTK(CC BY 4.0)。配布物のNOTICEに出典を載せる(`maid_cafe_enhance::NOTICE`)。
- モデルは約56MB(backbone 51.8MB + spec_head 4.2MB)。取得済みのSHA-256は`crates/maid-cafe-enhance/src/lib.rs`に固定。

## Rust移行計画 (2026-09-30〜)

ユーザー指示「アプリ本体もRust+RPoemで作り直して、早く」。現状のKotlin実装(`core`/`app`/`desktop`)を、テストを仕様として段階的にRustへ置き換える。

1. `core`のRust移植(`crates/`): 繰り返しルール・日本の祝日・Planner・Codec・セリフ番号・VoiceDsp(WSOLA/リサンプラ/EQ)。Kotlin側の`core`テスト75件と同じ検証をRustのテストで再現し、同じ入力で同じ出力になることを確認する。
2. Windows版をRustへ: トレイ常駐・スケジューラ・SAPI(WinRT/COM)・音声出力。UIの方式(RPoemのローカルWeb UIか、ネイティブUIか)は要相談。
3. Android版をRustへ: `core`をJNI/UniFFI経由で共有し、UI・アラーム登録・TTSだけ薄いKotlin(またはRustのAndroidバインディング)にする。
4. 置き換えが済んだ層から、Kotlin実装とGradleビルドを削除する。
