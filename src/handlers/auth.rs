// ============================================================
// handlers/auth.rs —— 认证相关的 HTTP 处理器
// ============================================================
// 【教学说明】
// 这个文件处理"与登录相关的 HTTP 请求"：
//   GET  /login          → 显示登录页（login_page）
//   POST /login          → 提交登录（login）
//   POST /logout         → 登出（logout）
//   GET  /admin/users    → 用户管理页（admin_users，仅管理员）
//   POST /admin/users    → 创建用户（admin_create_user，仅管理员）
//
// 与 auth.rs 的分工：
//   auth.rs     = 逻辑层（哈希、session 增删查）→ 不碰 HTTP
//   handlers/auth.rs = 表现层（解析表单、设 cookie、重定向）→ 只碰 HTTP
//
// 这样设计的好处：
//   1. 逻辑可以被测试（不用起服务器就能测哈希/session）
//   2. 以后换 HTTP 框架，逻辑不用改
// ============================================================

use axum::{
    extract::{Form, State},
    http::{HeaderMap, header::SET_COOKIE},
    response::{IntoResponse, Redirect},
};

use serde::Deserialize;

use crate::{AppState, auth, error::AppError, models::User};

// ============================================================
// 【教学：表单数据结构】
// serde 的 Deserialize 让这个结构体可以从 HTML 表单自动填充：
//   <form method="post">
//     <input name="username">
//     <input name="password" type="password">
//   </form>
// 提交后，axum 的 Form<LoginForm> 提取器会按 name 字段名匹配，
// 自动把 username/password 填进结构体。
// ============================================================
#[derive(Deserialize)]
pub struct LoginForm
{
    username: String,
    password: String,
}

/// 创建用户的表单（管理员用）
#[derive(Deserialize)]
pub struct CreateUserForm
{
    username: String,
    password: String,
    /// 是否设为管理员：表单 checkbox，值为 "1" 表示勾选
    is_admin: Option<String>,
}

// ============================================================
// 登录页（GET /login）
// ============================================================
/// 显示登录页面
///
/// 【教学：M1 先用内联 HTML，M2 换 askama 模板】
/// 返回类型 String 会被 axum 当作 text/html 响应。
pub async fn login_page() -> String
{
    // 【教学：HTML 表单】
    // method="post" → 提交时用 POST 方法
    // action="/login" → 提交到 /login 路由
    // name 属性 → 和 LoginForm 的字段名对应
    r#"<!DOCTYPE html>
<html lang="zh">
<head><meta charset="UTF-8"><title>登录</title></head>
<body>
  <h1>训练记录系统</h1>
  <h2>登录</h2>
  <form method="post" action="/login">
    <label>用户名 <input name="username" required></label><br>
    <label>密码 <input name="password" type="password" required></label><br>
    <button type="submit">登录</button>
  </form>
</body>
</html>"#
        .to_string()
}

// ============================================================
// 登录（POST /login）
// ============================================================
/// 处理登录表单提交
///
/// 流程：
///   1. 用表单里的用户名查用户
///   2. 校验密码（verify_password）
///   3. 成功 → 创建 session → 把 token 放进 cookie → 重定向到首页
///   4. 失败 → 返回登录页（带错误提示）
pub async fn login(
    State(state): State<AppState>,
    Form(form): Form<LoginForm>,
) -> Result<impl IntoResponse, AppError>
{
    // 1. 按用户名查用户
    //    【教学：fetch_optional】
    //    返回 Option<User>：查到 Some，查不到 None（不是报错！）
    let user: Option<User> = sqlx::query_as::<_, User>("SELECT * FROM users WHERE username = ?")
        .bind(&form.username)
        .fetch_optional(&state.pool)
        .await
        .map_err(AppError::Database)?;

    // 2. 用户不存在 → 登录失败
    //    为了防止攻击者探测"哪些用户名存在"，
    //    用户名不存在和密码错误返回一样的提示（模糊处理）
    let user = user.ok_or_else(|| AppError::Validation("用户名或密码错误".to_string()))?;

    // 3. 校验密码
    let ok = auth::verify_password(&form.password, &user.password_hash)?;
    if !ok
    {
        return Err(AppError::Validation("用户名或密码错误".to_string()));
    }

    // 4. 登录成功 → 创建 session
    let token = auth::create_session(&state.pool, user.id).await?;

    // 5. 把 token 放进 cookie，重定向到首页
    //    【教学：Set-Cookie 响应头】
    //    - Path=/  → 整个网站都有效（否则只对 /login 生效）
    //    - HttpOnly → 浏览器 JS 读不到（防 XSS 窃取）
    //    - SameSite=Lax → 防 CSRF（跨站请求伪造）
    //    - Max-Age=2592000 → 30 天后过期（和 session 过期时间一致）
    let cookie = format!("session={token}; Path=/; HttpOnly; SameSite=Lax; Max-Age=2592000");
    let mut headers = HeaderMap::new();
    headers.insert(
        SET_COOKIE,
        cookie
            .parse()
            .map_err(|_| AppError::Other("cookie 生成失败".to_string()))?,
    );

    tracing::info!("用户 {} 登录成功", user.username);
    Ok((headers, Redirect::to("/")))
}

// ============================================================
// 登出（POST /logout）
// ============================================================
/// 处理登出：销毁 session，清除 cookie，回到登录页
pub async fn logout(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError>
{
    // 1. 从请求头里取出 cookie（浏览器自动带的）
    //    【教学：HeaderMap 提取器】
    //    headers 参数会自动拿到整个请求头，我们只关心 Cookie 头
    if let Some(cookie_header) = headers.get(axum::http::header::COOKIE)
    {
        // 2. 解析出 session=xxx 的值
        //    【教学：字符串处理】
        //    "session=abc123" → 按 "=" 分割，取后半部分
        if let Some(token) = cookie_header.to_str().ok().and_then(|s| {
            s.split(';')
                .map(str::trim)
                .find(|part| part.starts_with("session="))
                .and_then(|part| part.split('=').nth(1))
        })
        {
            // 3. 销毁数据库里的 session
            auth::destroy_session(&state.pool, token).await?;
        }
    }

    // 4. 清除浏览器 cookie（Max-Age=0 立即过期）
    let cookie = "session=; Path=/; HttpOnly; SameSite=Lax; Max-Age=0";
    let mut headers = HeaderMap::new();
    headers.insert(SET_COOKIE, cookie.parse().unwrap());

    Ok((headers, Redirect::to("/login")))
}

// ============================================================
// 【教学：路由守卫（权限检查）】
// 为什么要有守卫？
//   没有守卫，任何人都能访问管理页。守卫 = "进门检查员"：
//   1. 检查你有没有带通行证（session cookie）
//   2. 检查通行证是否有效（token 在不在 sessions 表）
//   3. 检查你的权限够不够（是不是管理员）
//   任一不满足 → 拒绝访问。
//
// 实现方式（本项目）：写一个辅助函数，每个受保护 handler 开头调用。
// （更高级的做法是 axum 中间件，M2 再学。）
// ============================================================

/// 从请求头解析出 session token（没有则 None）
fn extract_token(headers: &HeaderMap) -> Option<String>
{
    headers
        .get(axum::http::header::COOKIE)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| {
            s.split(';')
                .map(str::trim)
                .find(|part| part.starts_with("session="))
                .and_then(|part| part.split('=').nth(1))
                .map(|t| t.to_string())
        })
}

/// 从请求头验证登录，返回当前用户（未登录 → Unauthorized）
async fn require_user(state: &AppState, headers: &HeaderMap) -> Result<User, AppError>
{
    let token = extract_token(headers).ok_or(AppError::Unauthorized)?;
    auth::get_user_by_session(&state.pool, &token).await
}

// ============================================================
// 用户管理页（GET /admin/users，仅管理员）
// ============================================================
/// 显示用户列表 + 创建用户表单（仅管理员可访问）
pub async fn admin_users(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<String, AppError>
{
    // 守卫：必须是登录的管理员
    let user = require_user(&state, &headers).await?;
    if !user.is_admin
    {
        return Err(AppError::Validation("需要管理员权限".to_string()));
    }

    // 查询所有用户列表
    let users: Vec<User> = sqlx::query_as::<_, User>("SELECT * FROM users ORDER BY id")
        .fetch_all(&state.pool)
        .await
        .map_err(AppError::Database)?;

    // 【教学：迭代器 + 适配器（函数式）】
    // 把用户列表拼成 HTML 表格行
    //   .iter()           → 遍历
    //   .map(|u| ...)     → 每个用户转成一行 HTML
    //   .collect::<Vec<_>>() → 收集成 Vec
    //   .join("\n")       → 用换行拼成一个大字符串
    let rows = users
        .iter()
        .map(|u| {
            format!(
                "<tr><td>{}</td><td>{}</td><td>{}</td></tr>",
                u.id,
                u.username,
                if u.is_admin
                {
                    "管理员"
                }
                else
                {
                    "普通用户"
                }
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    Ok(format!(
        r#"<!DOCTYPE html>
<html lang="zh">
<head><meta charset="UTF-8"><title>用户管理</title></head>
<body>
  <h1>用户管理</h1>
  <table border="1">
    <tr><th>ID</th><th>用户名</th><th>角色</th></tr>
    {rows}
  </table>
  <h2>创建新用户（邀请制）</h2>
  <form method="post" action="/admin/users">
    <label>用户名 <input name="username" required></label><br>
    <label>密码 <input name="password" type="password" required></label><br>
    <label><input type="checkbox" name="is_admin" value="1"> 设为管理员</label><br>
    <button type="submit">创建用户</button>
  </form>
  <p><a href="/">返回首页</a></p>
</body>
</html>"#
    ))
}

// ============================================================
// 创建用户（POST /admin/users，仅管理员）
// ============================================================
/// 管理员创建新用户（邀请制：只有管理员能创建账号）
pub async fn admin_create_user(
    State(state): State<AppState>,
    headers: HeaderMap,
    Form(form): Form<CreateUserForm>,
) -> Result<impl IntoResponse, AppError>
{
    // 守卫：必须是登录的管理员
    let admin = require_user(&state, &headers).await?;
    if !admin.is_admin
    {
        return Err(AppError::Validation("需要管理员权限".to_string()));
    }

    // 校验：用户名不能为空、密码不能太短
    if form.username.trim().is_empty()
    {
        return Err(AppError::Validation("用户名不能为空".to_string()));
    }
    if form.password.len() < 6
    {
        return Err(AppError::Validation("密码至少 6 位".to_string()));
    }

    // 哈希密码（不存明文！）
    let password_hash = auth::hash_password(&form.password)?;

    // is_admin：checkbox 勾选时值为 "1"
    let is_admin = form.is_admin.as_deref() == Some("1");

    // 插入用户
    // 【教学：INSERT 语句 + 参数绑定】
    // ? 占位符依次对应 bind 的参数：username, password_hash, is_admin
    sqlx::query("INSERT INTO users (username, password_hash, is_admin) VALUES (?, ?, ?)")
        .bind(&form.username)
        .bind(&password_hash)
        .bind(is_admin)
        .execute(&state.pool)
        .await
        .map_err(AppError::Database)?;

    tracing::info!("管理员 {} 创建了新用户 {}", admin.username, form.username);
    Ok(Redirect::to("/admin/users"))
}
