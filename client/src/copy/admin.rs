#[derive(Clone, Copy)]
pub struct Text
{
    pub title: &'static str,
    pub users: &'static str,
    pub create: &'static str,
    pub username: &'static str,
    pub password: &'static str,
    pub is_admin: &'static str,
    pub role_admin: &'static str,
    pub role_user: &'static str,
    pub created: &'static str,
    pub no_users: &'static str,
    pub backup_title: &'static str,
    pub download_db: &'static str,
    pub upload_db: &'static str,
    pub export_csv: &'static str,
    pub export_json: &'static str,
    pub backup_hint: &'static str,
}

pub fn text(lang: crate::i18n::Lang) -> Text
{
    match lang
    {
        crate::i18n::Lang::Zh => Text {
            title: "用户管理",
            users: "现有用户",
            create: "创建用户",
            username: "用户名",
            password: "密码（至少 6 位）",
            is_admin: "设为管理员",
            role_admin: "管理员",
            role_user: "普通用户",
            created: "用户已创建",
            no_users: "还没有用户",
            backup_title: "数据备份",
            download_db: "下载数据库",
            upload_db: "上传恢复数据库",
            export_csv: "导出 CSV",
            export_json: "导出 JSON",
            backup_hint: "上传恢复会自动备份当前数据库；下载与导出使用浏览器登录态。",
        },
        crate::i18n::Lang::En => Text {
            title: "Users",
            users: "Existing users",
            create: "Create user",
            username: "Username",
            password: "Password (6+ characters)",
            is_admin: "Administrator",
            role_admin: "Admin",
            role_user: "User",
            created: "User created",
            no_users: "No users yet",
            backup_title: "Backup",
            download_db: "Download database",
            upload_db: "Restore database",
            export_csv: "Export CSV",
            export_json: "Export JSON",
            backup_hint: "Restore backs up the current database first. Download/export uses the browser session.",
        },
    }
}
