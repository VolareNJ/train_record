#[derive(Clone, Copy)]
pub struct Text
{
    pub best: &'static str,
    pub best_2rm: &'static str,
    pub best_3rm: &'static str,
    pub trend: &'static str,
    pub recent: &'static str,
    pub no_records: &'static str,
    pub record_count: &'static str,
    pub weight_line: &'static str,
    pub one_rm_line: &'static str,
    pub two_rm_line: &'static str,
    pub chart_hint: &'static str,
}

pub fn text(lang: crate::i18n::Lang) -> Text
{
    match lang
    {
        crate::i18n::Lang::Zh => Text {
            best: "最佳 1RM",
            best_2rm: "最佳 2RM",
            best_3rm: "最佳 3RM",
            trend: "趋势",
            recent: "最近记录",
            no_records: "这个动作还没有记录",
            record_count: "条记录",
            weight_line: "实际重量",
            one_rm_line: "1RM",
            two_rm_line: "2RM",
            chart_hint: "最近 180 天",
        },
        crate::i18n::Lang::En => Text {
            best: "Best 1RM",
            best_2rm: "Best 2RM",
            best_3rm: "Best 3RM",
            trend: "Trend",
            recent: "Recent records",
            no_records: "No records for this exercise yet",
            record_count: "records",
            weight_line: "Weight",
            one_rm_line: "1RM",
            two_rm_line: "2RM",
            chart_hint: "Last 180 days",
        },
    }
}
