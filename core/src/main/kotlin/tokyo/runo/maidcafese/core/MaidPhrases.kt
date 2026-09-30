package tokyo.runo.maidcafese.core

/**
 * メイドのセリフ集(画面用)。読み上げの表記・抑揚・間と、喋る順の番号のルールは、Rust側(`maid-cafe-core`)が持つ。
 * ここに置くのは、画面に並べるid・表示文だけ(並びはこの順)。
 */
object MaidPhrases {
    data class Phrase(val id: String, val display: String)

    val all = listOf(
        Phrase("okite", "ご主人さま～、お～き～て～。今日も頑張って～"),
        Phrase("okaeri", "おかえりなさいませご主人様！"),
        Phrase("oishiku", "おいしくな～れ萌え萌えキュ～ン"),
        Phrase("meh", "メッ！ダメなんだぞこら！いつまでもクヨクヨしてないでメイドちゃんと一緒にやる気を出して頑張って行きましょう！"),
        Phrase("fight", "ファイト！ファイト！"),
        Phrase("excellent", "エクセレント！"),
        Phrase("perfect", "パーフェクト！"),
    )

    fun byId(id: String): Phrase? = all.firstOrNull { it.id == id }

    // ---- 喋る順の番号(画面で編集できる)。ルールはRust側: 番号の小さい順に喋る・同じ番号はかぶらない(assignが自動で振り直す)・欠番OK ----

    private fun toText(m: Map<String, Int>) = m.entries.joinToString(",") { "${it.key}:${it.value}" }

    private fun fromText(s: String): Map<String, Int> {
        val out = LinkedHashMap<String, Int>()
        for (p in s.split(",")) {
            val i = p.lastIndexOf(':')
            if (i > 0) p.substring(i + 1).toIntOrNull()?.let { out[p.substring(0, i)] = it }
        }
        return out
    }

    /** 保存された順序(選択順)から、1,2,3...の番号を振る。 */
    fun ranks(ids: Set<String>): Map<String, Int> = fromText(Native.phraseRanks(ids.joinToString(",")))

    /** 番号の小さい順に並べたid(これが読み上げ順)。 */
    fun ordered(numbers: Map<String, Int>): Set<String> =
        Native.phraseOrder(toText(numbers)).split(",").filterTo(LinkedHashSet()) { it.isNotEmpty() }

    /** セリフを選んだとき: 今の最大番号の次(末尾)に加える。何も無ければ1番。 */
    fun add(numbers: Map<String, Int>, id: String): Map<String, Int> = edit(numbers, "add", id, 0)

    /** セリフの選択を外したとき。ほかの番号はそのまま(欠番になる)。 */
    fun remove(numbers: Map<String, Int>, id: String): Map<String, Int> = edit(numbers, "remove", id, 0)

    /** [id]の番号を[n]にする。別のセリフが[n]を使っていたら、その持ち主は空いている最小の番号へ自動で振り直される。 */
    fun assign(numbers: Map<String, Int>, id: String, n: Int): Map<String, Int> = edit(numbers, "assign", id, n)

    private fun edit(numbers: Map<String, Int>, op: String, id: String, n: Int): Map<String, Int> =
        Native.phraseEdit(toText(numbers), op, id, n)?.let(::fromText) ?: numbers

    /** 未知のidを捨てる。順序(選択順)は保つ。 */
    fun known(ids: Set<String>): Set<String> = ids.filterTo(LinkedHashSet()) { byId(it) != null }
}
