//! アラーム・予定・設定・発火のデータモデルと、メイドのセリフ集。

use crate::recurrence::Schedule;
use chrono::NaiveDateTime;

/// 読み上げの声(`Maid`=メイドカフェ風、`DeepMale`=太くて低い男性)。実体は共有クレート`open-runo-voice`。
pub use open_runo_voice::VoiceStyle;

/// 鳴らし方。`Sound`=著作権フリー音、`Speech`=入力文章の読み上げ。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AlarmKind {
    Sound,
    Speech,
}

impl AlarmKind {
    pub fn name(self) -> &'static str {
        match self {
            AlarmKind::Sound => "SOUND",
            AlarmKind::Speech => "SPEECH",
        }
    }

    pub fn from_name(s: &str) -> Option<Self> {
        [AlarmKind::Sound, AlarmKind::Speech].into_iter().find(|k| k.name() == s)
    }
}

/// 同梱する音源(すべて本プロジェクトが数式から生成したCC0、`tools/gen-sounds`)。
pub struct Sound {
    pub id: &'static str,
    pub display_name: &'static str,
}

pub const SOUNDS: [Sound; 3] = [
    Sound { id: "chime", display_name: "チャイム" },
    Sound { id: "alarm", display_name: "アラーム(ピピピ)" },
    Sound { id: "melody", display_name: "やさしいメロディ" },
];
pub const DEFAULT_SOUND_ID: &str = "chime";

pub fn is_known_sound(id: &str) -> bool {
    SOUNDS.iter().any(|s| s.id == id)
}

/// ユーザーが登録する定型アラーム1件。`schedule`の予告設定(30分前チェック)が予告の有無を決める。
#[derive(Clone, Debug, PartialEq)]
pub struct AlarmEntry {
    pub id: String,
    pub label: String,
    pub schedule: Schedule,
    pub kind: AlarmKind,
    pub sound_id: String,
    pub text: String,
    pub voice: VoiceStyle,
    pub enabled: bool,
    /// 指定時刻に喋るメイドのセリフ([`MaidPhrases`]のid)。**喋る順**に並ぶ。
    pub phrases: Vec<String>,
    /// 予告時に喋るメイドのセリフ。喋る順。
    pub pre_phrases: Vec<String>,
    /// メイドちゃん2人でハモる(同じ文を2つの声で同時に)。
    pub harmony: bool,
}

impl AlarmEntry {
    pub fn new(id: &str, label: &str, schedule: Schedule, kind: AlarmKind) -> Self {
        AlarmEntry {
            id: id.into(),
            label: label.into(),
            schedule,
            kind,
            sound_id: DEFAULT_SOUND_ID.into(),
            text: String::new(),
            voice: VoiceStyle::Maid,
            enabled: true,
            phrases: Vec::new(),
            pre_phrases: Vec::new(),
            harmony: false,
        }
    }
}

/// カレンダー(端末同期分など)の予定1件。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CalendarEvent {
    pub id: String,
    pub title: String,
    pub start: NaiveDateTime,
}

/// カレンダー連動の設定。`pre_notice`が「30分前に予告」チェックボックス。
#[derive(Clone, Debug, PartialEq)]
pub struct CalendarSettings {
    pub enabled: bool,
    pub pre_notice: bool,
    pub pre_notice_minutes: u32,
    pub voice: VoiceStyle,
    pub phrases: Vec<String>,
    pub pre_phrases: Vec<String>,
    pub harmony: bool,
}

impl Default for CalendarSettings {
    fn default() -> Self {
        CalendarSettings {
            enabled: false,
            pre_notice: true,
            pre_notice_minutes: 30,
            voice: VoiceStyle::Maid,
            phrases: Vec::new(),
            pre_phrases: Vec::new(),
            harmony: false,
        }
    }
}

/// 読み上げの1区切り。`pitch`は声全体の音程への倍率、`rate`は話速への倍率、`gap_after_ms`は後ろの間(ミリ秒)。
#[derive(Clone, Debug, PartialEq)]
pub struct Segment {
    pub text: String,
    pub pitch: f64,
    pub rate: f32,
    pub gap_after_ms: i32,
}

impl Segment {
    pub fn plain(text: &str) -> Self {
        Segment { text: text.into(), pitch: 1.0, rate: 1.0, gap_after_ms: 0 }
    }
}

/// 1回の発火。`sound_id`が`Some`なら音を鳴らし、`speech`が`Some`なら`voice`で読み上げる。
#[derive(Clone, Debug, PartialEq)]
pub struct Occurrence {
    pub time: NaiveDateTime,
    pub key: String,
    pub title: String,
    pub sound_id: Option<String>,
    pub speech: Option<String>,
    pub voice: VoiceStyle,
    pub harmony: bool,
    /// 読み上げの区切り(文ごとの間・抑揚)。空なら`speech`を1文として読む。
    pub segments: Vec<Segment>,
}

/// メイドのセリフ1つ。`display`は画面表示、`spoken`はTTSが読み間違えにくい表記(長音・かな)。
/// `pitch`/`rate`/`gap_ms`はセリフごとの抑揚(音程倍率・話速倍率・後ろの間)。
pub struct Phrase {
    pub id: &'static str,
    pub display: &'static str,
    pub spoken: &'static str,
    pub pitch: f64,
    pub rate: f32,
    pub gap_ms: i32,
}

/// メイドのセリフ集(画面の並びはこの順)。
pub const PHRASES: [Phrase; 7] = [
    // 目覚まし用。喋る順は、ユーザーが付けた番号で決まる。
    Phrase {
        id: "okite",
        display: "ご主人さま～、お～き～て～。今日も頑張って～",
        spoken: "ご主人さまー、おーきーてー。今日も、がんばってー",
        pitch: 1.04,
        rate: 0.8,
        gap_ms: 350,
    },
    Phrase {
        id: "okaeri",
        display: "おかえりなさいませご主人様！",
        spoken: "おかえりなさいませ、ご主人様！",
        pitch: 1.0,
        rate: 0.95,
        gap_ms: 350,
    },
    Phrase {
        id: "oishiku",
        display: "おいしくな～れ萌え萌えキュ～ン",
        spoken: "おいしくなーれ、もえもえきゅーん！",
        pitch: 1.06,
        rate: 0.85,
        gap_ms: 400,
    },
    Phrase {
        id: "meh",
        display: "メッ！ダメなんだぞこら！いつまでもクヨクヨしてないでメイドちゃんと一緒にやる気を出して頑張って行きましょう！",
        spoken: "めっ！だめなんだぞ、こら！いつまでもくよくよしてないで、メイドちゃんと一緒に、やる気を出して、頑張って行きましょう！",
        pitch: 1.03,
        rate: 1.0,
        gap_ms: 350,
    },
    Phrase { id: "fight", display: "ファイト！ファイト！", spoken: "ファイト！ファイト！", pitch: 1.05, rate: 1.1, gap_ms: 250 },
    Phrase { id: "excellent", display: "エクセレント！", spoken: "エクセレント！", pitch: 1.05, rate: 1.0, gap_ms: 300 },
    Phrase { id: "perfect", display: "パーフェクト！", spoken: "パーフェクト！", pitch: 1.05, rate: 1.0, gap_ms: 300 },
];

/// メイドのセリフの検索と、**喋る順の番号**の操作。
///
/// 番号はセリフごとに持ち、喋る順は番号の小さい順。同じ番号は決してかぶらない([`assign`](MaidPhrases::assign)が
/// 自動で振り直す)。番号は欠番があってよい(外したセリフの番号は空く)。番号表は`(id, 番号)`の並び。
pub struct MaidPhrases;

/// `(セリフid, 番号)`の並び(挿入順を保つ)。
pub type Numbers = Vec<(String, i32)>;

impl MaidPhrases {
    pub fn by_id(id: &str) -> Option<&'static Phrase> {
        PHRASES.iter().find(|p| p.id == id)
    }

    /// 未知のidと重複を捨てる。順序(喋る順)は保つ。
    pub fn known(ids: &[String]) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        for id in ids {
            if Self::by_id(id).is_some() && !out.contains(id) {
                out.push(id.clone());
            }
        }
        out
    }

    /// 保存された順序(喋る順)から、1,2,3...の番号を振る。
    pub fn ranks(ids: &[String]) -> Numbers {
        ids.iter().enumerate().map(|(i, id)| (id.clone(), i as i32 + 1)).collect()
    }

    /// 番号の小さい順に並べたid(これが読み上げ順)。
    pub fn ordered(numbers: &Numbers) -> Vec<String> {
        let mut v: Vec<&(String, i32)> = numbers.iter().collect();
        v.sort_by_key(|(_, n)| *n); // 安定ソート: 同番号なら元の並びを保つ
        v.into_iter().map(|(id, _)| id.clone()).collect()
    }

    /// セリフを選んだとき: 今の最大番号の次(末尾)に加える。何も無ければ1番。
    pub fn add(numbers: &Numbers, id: &str) -> Numbers {
        if numbers.iter().any(|(i, _)| i == id) {
            return numbers.clone();
        }
        let next = numbers.iter().map(|(_, n)| *n).max().unwrap_or(0) + 1;
        let mut out = numbers.clone();
        out.push((id.to_string(), next));
        out
    }

    /// セリフの選択を外したとき。ほかの番号はそのまま(欠番になる)。
    pub fn remove(numbers: &Numbers, id: &str) -> Numbers {
        numbers.iter().filter(|(i, _)| i != id).cloned().collect()
    }

    /// `id`の番号を`n`にする。すでに別のセリフが`n`を使っていたら、後から入れた`id`が`n`を取り、
    /// 元の持ち主は**空いている最小の番号**(1から探す。1が使用中なら2以降)へ自動で振り直す。
    /// 選ばれていないid・1未満の番号は何もしない。結果に重複する番号は残らない。
    pub fn assign(numbers: &Numbers, id: &str, n: i32) -> Numbers {
        if n < 1 || !numbers.iter().any(|(i, _)| i == id) {
            return numbers.clone();
        }
        let holder = numbers.iter().find(|(i, v)| i != id && *v == n).map(|(i, _)| i.clone());
        let mut result: Numbers = numbers.iter().map(|(i, v)| (i.clone(), if i == id { n } else { *v })).collect();
        if let Some(h) = holder {
            let used: Vec<i32> = result.iter().filter(|(i, _)| *i != h).map(|(_, v)| *v).collect();
            let mut free = 1;
            while used.contains(&free) {
                free += 1;
            }
            for (i, v) in result.iter_mut() {
                if *i == h {
                    *v = free;
                }
            }
        }
        result
    }
}
