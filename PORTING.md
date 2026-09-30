# PORTING / 開発メモ (maid-cafe-se)

## 現状 (2026-09-30)
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
