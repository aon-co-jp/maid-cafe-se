# PORTING / 開発メモ (maid-cafe-se)

## 現状 (2026-09-30)
- リポジトリ新設、README(要件・方針)のみ。コード無し。

## 次回再開ポイント(優先順)
1. Androidプロジェクト雛形(Kotlin/Compose、minSdk要決定)
2. 繰り返しルールエンジン(純Kotlin、JVMユニットテスト先行): 毎日/平日/土日祝/曜日/第N週X曜/N週ごと
3. AlarmManager登録・再起動復元・音再生(CC0音源選定)
4. TTS読み上げ(低い男性声/メイド口調変換)
5. Googleカレンダー連動+30分前予告チェックボックス
6. 実機(Android)でのDoze/省電力下の動作検証

## 未決事項
- カレンダー取得方式: Google Calendar API(OAuth)か`CalendarContract`か
- 祝日データの取得元と更新方法
- 「メイドカフェ風」の口調変換ルール(辞書ベース)の範囲
- ライセンス
