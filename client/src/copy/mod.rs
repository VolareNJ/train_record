pub mod history;
pub mod library;
pub mod login;
pub mod phase;
pub mod plan;
pub mod record;
pub mod settings;
pub mod stats;
pub mod template;
pub mod today;

pub struct Common
{
    pub lang: crate::i18n::Lang,
    pub loading: &'static str,
    pub retry: &'static str,
    pub save: &'static str,
    pub saving: &'static str,
    pub delete: &'static str,
    pub edit: &'static str,
    pub remove: &'static str,
    pub yes: &'static str,
    pub no: &'static str,
    pub error: &'static str,
    pub date: &'static str,
    pub note: &'static str,
    pub name: &'static str,
    pub account: &'static str,
    pub logout: &'static str,
    pub language: &'static str,
    pub kg: &'static str,
    pub sets: &'static str,
    pub reps: &'static str,
    pub weight: &'static str,
    pub rest: &'static str,
    pub feeling: &'static str,
    pub strategy: &'static str,
    pub key_points: &'static str,
    pub completed: &'static str,
    pub plan: &'static str,
    pub today: &'static str,
    pub history: &'static str,
    pub library: &'static str,
    pub settings: &'static str,
    pub server: &'static str,
    pub admin: &'static str,
    pub open: &'static str,
    pub saved: &'static str,
    pub required: &'static str,
    pub not_set: &'static str,
    pub unknown: &'static str,
}

pub fn common(lang: crate::i18n::Lang) -> Common
{
    match lang
    {
        crate::i18n::Lang::Zh => Common {
            lang,
            loading: "加载中",
            retry: "重试",
            save: "保存",
            saving: "保存中",
            delete: "删除",
            edit: "编辑",
            remove: "移除",
            yes: "是",
            no: "否",
            error: "出错了",
            date: "日期",
            note: "备注",
            name: "名称",
            account: "账号",
            logout: "退出登录",
            language: "语言",
            kg: "公斤",
            sets: "组数",
            reps: "次数",
            weight: "重量",
            rest: "休息",
            feeling: "感受",
            strategy: "策略",
            key_points: "要领",
            completed: "已完成",
            plan: "计划",
            today: "今日",
            history: "历史",
            library: "动作",
            settings: "我的",
            server: "服务器",
            admin: "管理员",
            open: "打开",
            saved: "已保存",
            required: "必填",
            not_set: "未设置",
            unknown: "未知",
        },
        crate::i18n::Lang::En => Common {
            lang,
            loading: "Loading",
            retry: "Retry",
            save: "Save",
            saving: "Saving",
            delete: "Delete",
            edit: "Edit",
            remove: "Remove",
            yes: "Yes",
            no: "No",
            error: "Something went wrong",
            date: "Date",
            note: "Note",
            name: "Name",
            account: "Account",
            logout: "Sign out",
            language: "Language",
            kg: "kg",
            sets: "Sets",
            reps: "Reps",
            weight: "Weight",
            rest: "Rest",
            feeling: "Feeling",
            strategy: "Strategy",
            key_points: "Key points",
            completed: "Completed",
            plan: "Plan",
            today: "Today",
            history: "History",
            library: "Exercises",
            settings: "Me",
            server: "Server",
            admin: "Admin",
            open: "Open",
            saved: "Saved",
            required: "Required",
            not_set: "Not set",
            unknown: "Unknown",
        },
    }
}

impl Common
{
    pub fn days_text(&self, count: i64) -> String
    {
        match self.lang
        {
            crate::i18n::Lang::Zh => format!("已坚持 {count} 天"),
            crate::i18n::Lang::En => format!("{count} days in"),
        }
    }

    pub fn sets_reps(&self, sets: Option<i64>, reps: Option<i64>) -> String
    {
        match (sets, reps)
        {
            (Some(sets), Some(reps)) => format!("{sets}*{reps}"),
            (Some(sets), None) => format!("{sets}*{}", self.not_set),
            (None, Some(reps)) => format!("{}*{reps}", self.not_set),
            (None, None) => self.not_set.to_string(),
        }
    }

    pub fn weight_text(&self, weight: Option<f64>) -> String
    {
        match weight
        {
            Some(weight) => format!("{weight:.1} {}", self.kg),
            None => self.not_set.to_string(),
        }
    }

    pub fn number_text(&self, value: Option<i64>) -> String
    {
        match value
        {
            Some(value) => value.to_string(),
            None => self.not_set.to_string(),
        }
    }

    pub fn volume(&self, sets: i64, reps: i64, weight: f64) -> String
    {
        if weight > 0.0
        {
            format!("{sets}*{reps} @ {weight:.1} {}", self.kg)
        }
        else
        {
            format!("{sets}*{reps}")
        }
    }

    pub fn or_not_set(&self, value: &str) -> String
    {
        if value.trim().is_empty()
        {
            self.not_set.to_string()
        }
        else
        {
            value.to_string()
        }
    }
}
