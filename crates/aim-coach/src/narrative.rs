//! Fixed explanations: the product works fully without any LLM (04_TRAINING_LOOP.md section 7.4).
//! The LLM layer only rewrites these, and falls back to them when its answer fails validation.

use crate::diagnosis::Finding;
use crate::evaluation::Verdict;
use aim_analysis::SkillId;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Locale {
    Ar,
    En,
}

/// (headline, why it matters) for a finding.
pub fn finding_text(id: &str, locale: Locale) -> (&'static str, &'static str) {
    match (id, locale) {
        ("D1", Locale::En) => ("Slow to start", "You lose the duel before your hand moves."),
        ("D1", Locale::Ar) => ("بطء في البدء", "تخسر المواجهة قبل أن تبدأ يدك بالحركة."),
        ("D2", Locale::En) => (
            "Systematic overshoot",
            "Your first movement goes too far in a predictable way, which corrects quickly.",
        ),
        ("D2", Locale::Ar) => (
            "تجاوز منتظم للهدف",
            "حركتك الأولى تتجاوز الهدف بشكل متوقّع، وهذا يتصحّح بسرعة.",
        ),
        ("D3", Locale::En) => (
            "Inconsistent distance control",
            "This is not your sensitivity: the same flick lands differently each time.",
        ),
        ("D3", Locale::Ar) => (
            "عدم ثبات في المدى",
            "المشكلة ليست الحساسية؛ نفس الحركة تنتهي بشكل مختلف كل مرة.",
        ),
        ("D4", Locale::En) => (
            "Undershoot with many corrections",
            "You stop early, then crawl to the target.",
        ),
        ("D4", Locale::Ar) => ("توقف مبكر وتصحيحات كثيرة", "تتوقف قبل الهدف ثم تزحف إليه."),
        ("D5", Locale::En) => ("Jitter on target", "Your crosshair shakes around the head."),
        ("D5", Locale::Ar) => ("رجفة عند الهدف", "المؤشر يهتز حول الرأس."),
        ("D6", Locale::En) => (
            "Clicking too early",
            "You fire before arriving, so the first bullet is wasted.",
        ),
        ("D6", Locale::Ar) => ("نقر متسرّع", "تطلق قبل الوصول، فتضيع الرصاصة الأولى."),
        ("D7", Locale::En) => (
            "Hesitating before the shot",
            "Accurate but slow: in the game the enemy fires first.",
        ),
        ("D7", Locale::Ar) => ("تردد قبل الإطلاق", "دقيق لكن بطيء؛ في اللعبة سيسبقك الخصم."),
        ("D8", Locale::En) => (
            "Crosshair off head level",
            "Every duel needs a vertical fix first.",
        ),
        ("D8", Locale::Ar) => (
            "المؤشر ليس على مستوى الرأس",
            "تحتاج تصحيحاً عمودياً في كل مواجهة.",
        ),
        ("D9", Locale::En) => (
            "One weak direction",
            "Flicks in this direction are clearly slower than the rest.",
        ),
        ("D9", Locale::Ar) => (
            "ضعف في اتجاه معيّن",
            "حركتك في هذا الاتجاه أبطأ بوضوح من الباقي.",
        ),
        ("D10", Locale::En) => (
            "Good tracking, slow reaction",
            "You lose strafe duels when the enemy changes direction.",
        ),
        ("D10", Locale::Ar) => (
            "تتبّع جيد لكن تفاعل بطيء",
            "تخسر مبارزات الـ strafe عندما يغيّر الخصم اتجاهه.",
        ),
        ("D11", Locale::En) => (
            "Shooting while moving",
            "Your bullets are inaccurate no matter how good your aim is.",
        ),
        ("D11", Locale::Ar) => ("الإطلاق أثناء الحركة", "رصاصاتك غير دقيقة مهما كان تصويبك."),
        ("D13", Locale::En) => (
            "Trading accuracy for speed",
            "You give up precision to be fast.",
        ),
        ("D13", Locale::Ar) => ("سرعة على حساب الدقة", "تضحي بالدقة لتكون سريعاً."),
        ("D14", Locale::En) => (
            "Too cautious",
            "You are precise but give up too much speed.",
        ),
        ("D14", Locale::Ar) => ("حذر زائد", "دقيق لكن تتنازل عن سرعة كثيرة."),
        (w, Locale::En) if w.starts_with("W_") => (
            "One of your weakest skills",
            "No single habit stands out, so this block raises your lowest score.",
        ),
        (w, Locale::Ar) if w.starts_with("W_") => (
            "من أضعف مهاراتك",
            "ماكو عادة وحدة واضحة، فهذي الكتلة ترفع أقل درجة عندك.",
        ),
        (_, Locale::En) => ("Finding", ""),
        (_, Locale::Ar) => ("ملاحظة", ""),
    }
}

pub fn skill_label(skill: SkillId, locale: Locale) -> &'static str {
    let (ar, en) = match skill {
        SkillId::S01Reaction => ("رد الفعل", "Reaction"),
        SkillId::S02FlickSpeed => ("سرعة الفليك", "Flick speed"),
        SkillId::S03FlickPrecision => ("دقة الفليك", "Flick precision"),
        SkillId::S04GainControl => ("التحكم بالمدى", "Distance control"),
        SkillId::S05MicroCorrection => ("التصحيح الدقيق", "Micro correction"),
        SkillId::S06TrackSmooth => ("التتبع السلس", "Smooth tracking"),
        SkillId::S07TrackReactive => ("التتبع التفاعلي", "Reactive tracking"),
        SkillId::S08TargetSwitch => ("التبديل بين الأهداف", "Target switching"),
        SkillId::S09ClickTiming => ("توقيت النقر", "Click timing"),
        SkillId::S10HeadLevel => ("مستوى الرأس", "Head level"),
        SkillId::S11DirectionalBalance => ("توازن الاتجاهات", "Directional balance"),
        SkillId::S12CounterStrafe => ("الإيقاف قبل الإطلاق", "Counter-strafe"),
        SkillId::S13Consistency => ("الثبات", "Consistency"),
    };
    if locale == Locale::Ar {
        ar
    } else {
        en
    }
}

/// A drill or test name for reports; unknown ids come back as written.
pub fn drill_label(id: &str, locale: Locale) -> &str {
    let names: Option<(&str, &str)> = match id {
        "warmup" => Some(("إحماء", "Warm-up")),
        "deceleration_ladder" => Some(("سلّم التخفيف", "Deceleration ladder")),
        "commit_flick" => Some(("فليك كامل", "Commit flick")),
        "consistency_grid" => Some(("شبكة الثبات", "Consistency grid")),
        "micro_hold" => Some(("تثبيت دقيق", "Micro hold")),
        "confirm_click" => Some(("نقر بعد الوصول", "Confirm click")),
        "timed_kill" => Some(("قتل موقوت", "Timed kill")),
        "head_level_hold" => Some(("ثبات مستوى الرأس", "Head level hold")),
        "peek_line" => Some(("خط الظهور", "Peek line")),
        "directional_flick" => Some(("فليك الاتجاهات", "Directional flick")),
        "strafe_track_reactive" => Some(("تتبع strafe", "Reactive strafe")),
        "smooth_track" => Some(("تتبع سلس", "Smooth track")),
        "reflex_dot" => Some(("نقطة رد الفعل", "Reflex dot")),
        "precision_first" => Some(("الدقة أولاً", "Precision first")),
        "speed_push" => Some(("دفع السرعة", "Speed push")),
        "cs_drill" => Some(("تمرين الإيقاف", "Counter-strafe drill")),
        "practice" => Some(("جلسة مخصصة", "Custom session")),
        "flick_static" => Some(("الفليك الثابت", "Static flick")),
        "micro_precision" => Some(("الدقة الصغيرة", "Micro precision")),
        "head_level_peeks" => Some(("الظهور على مستوى الرأس", "Head-level peeks")),
        "strafe_track" => Some(("تتبع strafe", "Strafe tracking")),
        "multi_switch" => Some(("التبديل بين الأهداف", "Target switching")),
        "counter_strafe" => Some(("الإيقاف قبل الإطلاق", "Counter-strafe")),
        _ => None,
    };
    match names {
        Some((ar, _)) if locale == Locale::Ar => ar,
        Some((_, en)) => en,
        None => id,
    }
}

pub fn explain_finding(finding: &Finding, locale: Locale) -> String {
    let (headline, why) = finding_text(&finding.id, locale);
    format!("{headline}: {why}")
}

pub fn verdict_text(verdict: Verdict, locale: Locale) -> &'static str {
    match (verdict, locale) {
        (Verdict::Improved, Locale::En) => "Confirmed improvement",
        (Verdict::Improved, Locale::Ar) => "تحسّن مؤكّد",
        (Verdict::ProbablyImproved, Locale::En) => "Probable improvement, confirm on the next test",
        (Verdict::ProbablyImproved, Locale::Ar) => "تحسّن محتمل، يحتاج تأكيد في الاختبار القادم",
        (Verdict::NoChange, Locale::En) => "No measurable change",
        (Verdict::NoChange, Locale::Ar) => "لا تغيير قابل للقياس",
        (Verdict::Regressed, Locale::En) => "Regression",
        (Verdict::Regressed, Locale::Ar) => "تراجع",
        (Verdict::ProbablyRegressed, Locale::En) => "Possible regression, not confirmed",
        (Verdict::ProbablyRegressed, Locale::Ar) => "تراجع محتمل، غير مؤكّد",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_rule_has_text_in_both_languages() {
        for id in [
            "D1", "D2", "D3", "D4", "D5", "D6", "D7", "D8", "D9", "D10", "D11", "D13", "D14",
        ] {
            for locale in [Locale::Ar, Locale::En] {
                assert!(!finding_text(id, locale).1.is_empty(), "{id}");
            }
        }
    }
}
