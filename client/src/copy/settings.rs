#[derive(Clone, Copy)]
pub struct Text
{
    pub body_weight: &'static str,
    pub body_weight_hint: &'static str,
    pub version: &'static str,
    pub admin_tools: &'static str,
    pub users: &'static str,
    pub backup: &'static str,
    pub about: &'static str,
    pub training: &'static str,
    pub phases_plans: &'static str,
    pub about_text: &'static str,
    pub saved: &'static str,
    pub invalid_weight: &'static str,
    pub sign_out_confirm: &'static str,
}

pub fn text(lang: crate::i18n::Lang) -> Text
{
    match lang
    {
        crate::i18n::Lang::Zh => Text {
            body_weight: "体重",
            body_weight_hint: "自重动作换算使用；留空表示清除",
            version: "客户端版本",
            admin_tools: "管理工具",
            users: "用户管理",
            backup: "数据备份",
            about: "关于",
            training: "训练管理",
            phases_plans: "阶段、模板与计划",
            about_text: "训练记录手机客户端，使用 iced 构建并部署为 WebAssembly",
            saved: "已保存",
            invalid_weight: "体重必须是 0 到 500 之间的数字",
            sign_out_confirm: "确定退出登录？",
        },
        crate::i18n::Lang::En => Text {
            body_weight: "Body weight",
            body_weight_hint: "Used by assistance mode; leave empty to clear",
            version: "Client version",
            admin_tools: "Admin tools",
            users: "Users",
            backup: "Backup",
            about: "About",
            training: "Training management",
            phases_plans: "Phases, templates and plans",
            about_text: "Mobile training client built with iced and deployed as WebAssembly",
            saved: "Saved",
            invalid_weight: "Body weight must be between 0 and 500",
            sign_out_confirm: "Sign out?",
        },
    }
}
