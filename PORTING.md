# PORTING / 開発メモ (maid-cafe-se)

## 現状 (2026-09-30)
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

## 未決事項
- 「メイドカフェ風」の口調変換ルールの拡充範囲
- ライセンス
