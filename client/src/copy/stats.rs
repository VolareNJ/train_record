#[derive(Clone, Copy)]
pub struct Text
{
    pub best: &'static str,
    pub trend: &'static str,
    pub recent: &'static str,
    pub no_records: &'static str,
    pub record_count: &'static str,
}

pub fn text(lang: crate::i18n::Lang) -> Text
{
    match lang
    {
        crate::i18n::Lang::Zh => Text {
            best: "最佳 1RM",
            trend: "1RM 趋势",
            recent: "最近记录",
            no_records: "这个动作还没有记录",
            record_count: "条记录",
        },
        crate::i18n::Lang::En => Text {
            best: "Best 1RM",
            trend: "1RM trend",
            recent: "Recent records",
            no_records: "No records for this exercise yet",
            record_count: "records",
        },
    }
}
