#[derive(Clone, Copy)]
pub struct Text
{
    pub title: &'static str,
    pub subtitle: &'static str,
    pub username: &'static str,
    pub password: &'static str,
    pub submit: &'static str,
    pub signing_in: &'static str,
    pub hint: &'static str,
    pub remember: &'static str,
}

pub fn text(lang: crate::i18n::Lang) -> Text
{
    match lang
    {
        crate::i18n::Lang::Zh => Text {
            title: "训练记录",
            subtitle: "手机训练，随时落库",
            username: "用户名",
            password: "密码",
            submit: "登录",
            signing_in: "登录中",
            hint: "使用管理员分配的账号登录",
            remember: "登录状态保持三十天",
        },
        crate::i18n::Lang::En => Text {
            title: "Train Record",
            subtitle: "Log every set from your phone",
            username: "Username",
            password: "Password",
            submit: "Sign in",
            signing_in: "Signing in",
            hint: "Use the account provided by the administrator",
            remember: "Your session lasts thirty days",
        },
    }
}
