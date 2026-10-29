#[derive(Clone, Copy)]
pub struct Text
{
    pub title: &'static str,
    pub new_phase: &'static str,
    pub active: &'static str,
    pub archived: &'static str,
    pub archive: &'static str,
    pub unarchive: &'static str,
    pub templates: &'static str,
    pub plans: &'static str,
    pub empty: &'static str,
    pub create_title: &'static str,
    pub edit_title: &'static str,
    pub start_date: &'static str,
    pub archive_confirm: &'static str,
    pub archived_hint: &'static str,
    pub start_hint: &'static str,
}

pub fn text(lang: crate::i18n::Lang) -> Text
{
    match lang
    {
        crate::i18n::Lang::Zh => Text {
            title: "训练阶段",
            new_phase: "新建阶段",
            active: "进行中",
            archived: "已归档",
            archive: "归档",
            unarchive: "启用",
            templates: "模板",
            plans: "计划",
            empty: "还没有阶段，先建一个开始训练",
            create_title: "新建阶段",
            edit_title: "编辑阶段",
            start_date: "开始日期",
            archive_confirm: "归档后本阶段只读，确定？",
            archived_hint: "已归档阶段只读，可查看历史",
            start_hint: "格式 YYYY-MM-DD，留空表示未设置",
        },
        crate::i18n::Lang::En => Text {
            title: "Phases",
            new_phase: "New phase",
            active: "Active",
            archived: "Archived",
            archive: "Archive",
            unarchive: "Reactivate",
            templates: "Templates",
            plans: "Plans",
            empty: "No phase yet; create one to start training",
            create_title: "New phase",
            edit_title: "Edit phase",
            start_date: "Start date",
            archive_confirm: "Archived phases are read-only. Continue?",
            archived_hint: "Archived phases are read-only; history stays available",
            start_hint: "Format YYYY-MM-DD; leave empty for unset",
        },
    }
}
