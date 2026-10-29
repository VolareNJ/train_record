#[derive(Clone, Copy)]
pub struct Text
{
    pub prev: &'static str,
    pub next: &'static str,
    pub weekdays: [&'static str; 7],
    pub training_days: &'static str,
    pub no_training: &'static str,
    pub day_title: &'static str,
    pub no_records: &'static str,
}

pub fn text(lang: crate::i18n::Lang) -> Text
{
    match lang
    {
        crate::i18n::Lang::Zh => Text {
            prev: "上月",
            next: "下月",
            weekdays: ["一", "二", "三", "四", "五", "六", "日"],
            training_days: "训练日",
            no_training: "本月还没有训练记录",
            day_title: "当天记录",
            no_records: "这天没有记录",
        },
        crate::i18n::Lang::En => Text {
            prev: "Prev",
            next: "Next",
            weekdays: ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"],
            training_days: "Training days",
            no_training: "No training this month",
            day_title: "Day records",
            no_records: "No records on this day",
        },
    }
}
