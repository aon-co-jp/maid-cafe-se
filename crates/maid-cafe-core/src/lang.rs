//! 読み上げの言語。固定のセリフと定型文を10言語で内蔵する(翻訳サービスは使わない=通信もAPIキーも不要)。
//!
//! **翻訳はAI(Claude)が下訳したもので、各言語のネイティブによる確認はしていない。** 自由に入力した文章は翻訳しない
//! (入力したまま、その言語の音声で読み上げる)。

use crate::model::VoiceStyle;

/// 対応言語(コード, 自国語の名前, 英語名)。先頭が既定の日本語。
pub const LANGS: [(&str, &str, &str); 10] = [
    ("ja", "日本語", "Japanese"),
    ("en", "English", "English"),
    ("zh", "中文(简体)", "Chinese"),
    ("ko", "한국어", "Korean"),
    ("it", "Italiano", "Italian"),
    ("fr", "Français", "French"),
    ("de", "Deutsch", "German"),
    ("ru", "Русский", "Russian"),
    ("fa", "فارسی", "Persian (Iran)"),
    ("ar", "العربية", "Arabic"),
];

pub const DEFAULT_LANG: &str = "ja";

pub fn is_known_lang(code: &str) -> bool {
    LANGS.iter().any(|l| l.0 == code)
}

/// 未知のコードは日本語にする。
pub fn normalize(code: &str) -> &'static str {
    LANGS.iter().find(|l| l.0 == code).map(|l| l.0).unwrap_or(DEFAULT_LANG)
}

/// 言語コードの位置(表の並び)。
fn idx(code: &str) -> usize {
    LANGS.iter().position(|l| l.0 == code).unwrap_or(0)
}

/// 固定セリフの、その言語での読み上げ文。`ja`は`MaidPhrases`側(長音などTTS向けの表記)を使うので、ここは日本語以外。
/// 並びは`LANGS`と同じ(ja, en, zh, ko, it, fr, de, ru, fa, ar)。日本語の位置は空。
const PHRASES: [(&str, [&str; 10]); 7] = [
    (
        "okite",
        [
            "",
            "Master, wake up, please. Do your best today too!",
            "主人，起床啦。今天也要加油哦！",
            "주인님, 일어나세요. 오늘도 힘내세요!",
            "Padrone, svegliati. Anche oggi fai del tuo meglio!",
            "Maître, réveillez-vous. Courage pour aujourd'hui aussi !",
            "Herr, aufwachen. Gib auch heute dein Bestes!",
            "Господин, проснитесь. И сегодня у вас всё получится!",
            "ارباب، بیدار شوید. امروز هم تلاش کنید!",
            "سيدي، استيقظ. اجتهد اليوم أيضًا!",
        ],
    ),
    (
        "okaeri",
        [
            "",
            "Welcome home, Master!",
            "欢迎回来，主人！",
            "어서 오세요, 주인님!",
            "Bentornato a casa, Padrone!",
            "Bienvenue à la maison, Maître !",
            "Willkommen zu Hause, Herr!",
            "Добро пожаловать домой, господин!",
            "به خانه خوش آمدید، ارباب!",
            "مرحبًا بعودتك إلى المنزل يا سيدي!",
        ],
    ),
    (
        "oishiku",
        [
            "",
            "Make it delicious! Moe moe kyun!",
            "变得好吃吧！萌萌哒！",
            "맛있어져라! 모에모에 큥!",
            "Diventa delizioso! Moe moe kyun!",
            "Deviens délicieux ! Moe moe kyun !",
            "Werde lecker! Moe moe kyun!",
            "Стань вкусным! Мое-мое кюн!",
            "خوشمزه شو! موئه موئه کیون!",
            "كن لذيذًا! موي موي كيون!",
        ],
    ),
    (
        "meh",
        [
            "",
            "No! That's not allowed! Stop dwelling on it, cheer up with your maid, and let's do our best!",
            "不行哦！不要总是闷闷不乐，和女仆一起打起精神加油吧！",
            "안 돼요! 언제까지 끙끙 앓고 있을 거예요? 메이드와 함께 힘을 내서 열심히 해봐요!",
            "No! Non va bene! Smettila di abbatterti e riprendi coraggio insieme alla tua cameriera, forza!",
            "Non ! Ce n'est pas bien ! Arrêtez de ruminer et retrouvez de l'énergie avec votre servante, allons-y !",
            "Nein! Das geht nicht! Grübele nicht ewig, nimm mit deinem Dienstmädchen neuen Mut, und los geht's!",
            "Нет! Так нельзя! Не унывайте, соберитесь вместе с вашей горничной, и вперёд!",
            "نه! این درست نیست! دیگر غصه نخور و با خدمتکارت انرژی بگیر و تلاش کنیم!",
            "لا! هذا لا يجوز! كفّ عن الحزن وتشجّع مع خادمتك، هيا نبذل جهدنا!",
        ],
    ),
    (
        "fight",
        [
            "",
            "Fight! Fight!",
            "加油！加油！",
            "파이팅! 파이팅!",
            "Forza! Forza!",
            "Courage ! Courage !",
            "Los geht's! Los geht's!",
            "Вперёд! Вперёд!",
            "تلاش کن! تلاش کن!",
            "هيا! هيا!",
        ],
    ),
    (
        "excellent",
        ["", "Excellent!", "太棒了！", "훌륭해요!", "Eccellente!", "Excellent !", "Ausgezeichnet!", "Отлично!", "عالی!", "ممتاز!"],
    ),
    (
        "perfect",
        ["", "Perfect!", "完美！", "완벽해요!", "Perfetto!", "Parfait !", "Perfekt!", "Идеально!", "کامل!", "مثالي!"],
    ),
];

/// 固定セリフの、日本語以外での読み上げ文。日本語(または未対応)なら`None`。
pub fn phrase_spoken(id: &str, lang: &str) -> Option<&'static str> {
    let i = idx(lang);
    if i == 0 {
        return None;
    }
    PHRASES.iter().find(|p| p.0 == id).map(|p| p.1[i]).filter(|s| !s.is_empty())
}

/// 指定時刻の基本メッセージ(`text`はユーザーの入力そのまま)。
const ALARM_MAID: [&str; 10] = [
    "",
    "Master, it's time. {t}. Please don't forget.",
    "主人，时间到了哦。{t}。请不要忘记哦",
    "주인님, 시간이에요. {t}. 잊지 말아 주세요",
    "Padrone, è l'ora. {t}. Non dimenticare, per favore",
    "Maître, il est l'heure. {t}. N'oubliez pas, s'il vous plaît",
    "Herr, es ist Zeit. {t}. Bitte vergiss es nicht",
    "Господин, пора. {t}. Пожалуйста, не забудьте",
    "ارباب، وقتش رسیده. {t}. لطفاً فراموش نکنید",
    "سيدي، حان الوقت. {t}. من فضلك لا تنسَ",
];
const ALARM_DEEP: [&str; 10] = [
    "",
    "Time. {t}",
    "时间到了。{t}",
    "시간이다. {t}",
    "È l'ora. {t}",
    "C'est l'heure. {t}",
    "Es ist Zeit. {t}",
    "Время. {t}",
    "وقتشه. {t}",
    "حان الوقت. {t}",
];
/// 予告(`{m}`=分、`{t}`=名前)。
const PRE_MAID: [&str; 10] = [
    "",
    "Master, {m} minutes until {t}.",
    "主人，还有{m}分钟就到{t}的时间了哦",
    "주인님, {m}분 후에 {t} 시간이에요",
    "Padrone, mancano {m} minuti a {t}",
    "Maître, il reste {m} minutes avant {t}",
    "Herr, noch {m} Minuten bis {t}",
    "Господин, до {t} осталось {m} минут",
    "ارباب، {m} دقیقه تا {t} مانده",
    "سيدي، تبقّى {m} دقيقة على {t}",
];
const PRE_DEEP: [&str; 10] = [
    "",
    "{m} minutes until {t}.",
    "还有{m}分钟，{t}的时间",
    "{m}분 후, {t} 시간이다",
    "Mancano {m} minuti a {t}",
    "Encore {m} minutes avant {t}",
    "Noch {m} Minuten bis {t}",
    "До {t} {m} минут",
    "{m} دقیقه تا {t}",
    "تبقّى {m} دقيقة على {t}",
];

/// 日本語以外の指定時刻の基本メッセージ。
pub fn alarm_message(text: &str, voice: VoiceStyle, lang: &str) -> Option<String> {
    let i = idx(lang);
    if i == 0 {
        return None;
    }
    let t = match voice {
        VoiceStyle::Maid => ALARM_MAID[i],
        VoiceStyle::DeepMale => ALARM_DEEP[i],
    };
    Some(t.replace("{t}", text.trim()))
}

/// 日本語以外の予告メッセージ。
pub fn pre_notice_message(title: &str, minutes: u32, voice: VoiceStyle, lang: &str) -> Option<String> {
    let i = idx(lang);
    if i == 0 {
        return None;
    }
    let t = match voice {
        VoiceStyle::Maid => PRE_MAID[i],
        VoiceStyle::DeepMale => PRE_DEEP[i],
    };
    Some(t.replace("{m}", &minutes.to_string()).replace("{t}", title))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_language_has_every_phrase_and_template() {
        for (i, (code, name, _)) in LANGS.iter().enumerate().skip(1) {
            assert!(!name.is_empty());
            for (id, texts) in PHRASES.iter() {
                assert!(!texts[i].is_empty(), "{id} が {code} に無い");
            }
            for table in [&ALARM_MAID, &ALARM_DEEP] {
                assert!(table[i].contains("{t}"), "{code}");
            }
            for table in [&PRE_MAID, &PRE_DEEP] {
                assert!(table[i].contains("{m}") && table[i].contains("{t}"), "{code}");
            }
        }
        assert!(PHRASES.iter().all(|p| p.1[0].is_empty()));
    }

    #[test]
    fn normalize_and_lookup() {
        assert_eq!("ja", normalize("xx"));
        assert_eq!("fa", normalize("fa"));
        assert_eq!(None, phrase_spoken("okite", "ja"));
        assert_eq!(Some("Perfect!"), phrase_spoken("perfect", "en"));
        assert_eq!(None, phrase_spoken("bogus", "en"));
        assert_eq!("Master, 30 minutes until Meeting.", pre_notice_message("Meeting", 30, VoiceStyle::Maid, "en").unwrap());
        assert_eq!("Time. pills", alarm_message(" pills ", VoiceStyle::DeepMale, "en").unwrap());
        assert_eq!(None, alarm_message("x", VoiceStyle::Maid, "ja"));
    }
}
