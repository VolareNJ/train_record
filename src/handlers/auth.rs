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
//
// 📌 阶段要求：M1 你来实现本文件剩余的函数。
//   完整实现已备份在 docs/learning_path/M1_ref/handlers_auth_ref.rs，
//   实现完成后对照检查（不要提前看）。
// ============================================================

// 【教学：本文件用到的导入】
// 下面每个导入都会在某个函数里用到，实现时自然会用上。
// （骨架阶段有 unused 警告是正常的，实现完就消失了。）
use axum::{
    extract::{Form, State},
    http::{HeaderMap, header::SET_COOKIE}, // SET_COOKIE：设置响应头用（login/logout）
    response::Redirect,                    // 重定向：登录成功跳首页、登出跳登录页
};
use serde::Deserialize; // 让表单结构体能从 HTML 表单自动填充

use crate::{AppState, auth, error::AppError, models::User};
//   AppState   = 共享数据（连接池等），State(state) 提取器用它
//   auth       = 上一节实现的认证逻辑（hash/verify/create_session/...）
//   AppError   = 统一的错误类型，? 运算符自动转
//   User       = 用户表结构体，查询结果转成它

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
/// 完整实现已在参考文件里，直接对照抄写即可。
pub async fn login_page() -> String
{
    // TODO(M1): 学生实现
    // 提示：返回一个 HTML 字符串，包含 form 表单
    //   <form method="post" action="/login">
    //     <label>用户名 <input name="username" required></label><br>
    //     <label>密码 <input name="password" type="password" required></label><br>
    //     <button type="submit">登录</button>
    //   </form>
    unimplemented!("M1 学生实现：登录页")
}

// ============================================================
// 登录（POST /login）
// ============================================================
/// 处理登录表单提交
///
/// 流程：
///   1. 用表单里的用户名查用户（sqlx 查询 + fetch_optional）
///   2. 校验密码（auth::verify_password）
///   3. 成功 → auth::create_session 创建 session → 把 token 放进 cookie → 重定向到 /
///   4. 失败 → 返回 Err(AppError::Validation("用户名或密码错误"))
///
/// 【教学：防用户名探测】
/// 用户不存在和密码错误返回一样的提示，防止攻击者探测
/// "哪些用户名是注册过的"。
///
/// 【实现步骤】
/// 1. 查用户：
///    sqlx::query_as::<_, User>("SELECT * FROM users WHERE username = ?")
///        .bind(&form.username)
///        .fetch_optional(&state.pool)
///        .await
///        .map_err(AppError::Database)?
/// 2. 用户不存在 → .ok_or_else(|| AppError::Validation("用户名或密码错误".to_string()))?
/// 3. 校验密码 → 失败返回同样的 Validation 错误
/// 4. 创建 session：let token = auth::create_session(&state.pool, user.id).await?;
/// 5. 组装 cookie：
///    format!("session={token}; Path=/; HttpOnly; SameSite=Lax; Max-Age=2592000")
///    （Max-Age=2592000 秒 = 30 天，与 session 过期一致）
/// 6. 放入响应头：
///    let mut headers = HeaderMap::new();
///    headers.insert(SET_COOKIE, cookie.parse().map_err(...)?);
/// 7. 返回 (headers, Redirect::to("/"))
/// 返回类型说明：登录成功返回「响应头 + 重定向」的元组
/// （响应头里放 Set-Cookie，重定向到首页）
pub async fn login(
    State(state): State<AppState>,
    Form(form): Form<LoginForm>,
) -> Result<(HeaderMap, Redirect), AppError>
{
    // TODO(M1): 学生实现（步骤见上方注释）
    unimplemented!("M1 学生实现：登录")
}

// ============================================================
// 登出（POST /logout）
// ============================================================
/// 处理登出：销毁 session，清除 cookie，回到登录页
///
/// 【实现步骤】
/// 1. 从请求头里取 Cookie 头：headers.get(axum::http::header::COOKIE)
/// 2. 解析出 session=xxx 的 token（可复用下面的 extract_token 辅助函数）
/// 3. 有 token → auth::destroy_session(&state.pool, token).await?
/// 4. 设置清除 cookie：let cookie = "session=; Path=/; HttpOnly; SameSite=Lax; Max-Age=0";
///    （Max-Age=0 = 立即过期，浏览器删除这个 cookie）
/// 5. 返回 (headers, Redirect::to("/login"))
/// 返回类型说明：登出返回「响应头 + 重定向」的元组
/// （响应头里放清除 cookie，重定向到登录页）
pub async fn logout(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<(HeaderMap, Redirect), AppError>
{
    // TODO(M1): 学生实现（步骤见上方注释）
    unimplemented!("M1 学生实现：登出")
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
// 实现方式（本项目）：写辅助函数，每个受保护 handler 开头调用。
// （更高级的做法是 axum 中间件，M2 再学。）
// ============================================================

/// 从请求头解析出 session token（没有则 None）
///
/// 【实现步骤】
/// headers.get(axum::http::header::COOKIE)
///     .and_then(|v| v.to_str().ok())
///     .and_then(|s| {
///         s.split(';')
///             .map(str::trim)
///             .find(|part| part.starts_with("session="))
///             .and_then(|part| part.split('=').nth(1))
///             .map(|t| t.to_string())
///     })
fn extract_token(headers: &HeaderMap) -> Option<String>
{
    // TODO(M1): 学生实现（步骤见上方注释）
    unimplemented!("M1 学生实现：解析 cookie token")
}

/// 从请求头验证登录，返回当前用户（未登录 → Unauthorized）
///
/// 【实现步骤】
/// 1. let token = extract_token(headers).ok_or(AppError::Unauthorized)?;
/// 2. auth::get_user_by_session(&state.pool, &token).await
async fn require_user(state: &AppState, headers: &HeaderMap) -> Result<User, AppError>
{
    // TODO(M1): 学生实现（步骤见上方注释）
    unimplemented!("M1 学生实现：路由守卫")
}

// ============================================================
// 用户管理页（GET /admin/users，仅管理员）
// ============================================================
/// 显示用户列表 + 创建用户表单（仅管理员可访问）
///
/// 【实现步骤】
/// 1. 守卫：let user = require_user(&state, &headers).await?;
/// 2. 管理员检查：if !user.is_admin { return Err(AppError::Validation("需要管理员权限".to_string())); }
/// 3. 查所有用户：
///    sqlx::query_as::<_, User>("SELECT * FROM users ORDER BY id")
///        .fetch_all(&state.pool).await.map_err(AppError::Database)?
/// 4. 迭代器拼 HTML 表格行：
///    users.iter()
///        .map(|u| format!("<tr><td>{}</td><td>{}</td><td>{}</td></tr>",
///            u.id, u.username, if u.is_admin { "管理员" } else { "普通用户" }))
///        .collect::<Vec<_>>().join("\n")
/// 5. 用 format! 拼完整 HTML 页面返回
pub async fn admin_users(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<String, AppError>
{
    // TODO(M1): 学生实现（步骤见上方注释）
    unimplemented!("M1 学生实现：用户管理页")
}

// ============================================================
// 创建用户（POST /admin/users，仅管理员）
// ============================================================
/// 管理员创建新用户（邀请制：只有管理员能创建账号）
///
/// 【实现步骤】
/// 1. 守卫：require_user + is_admin 检查（同 admin_users）
/// 2. 校验：用户名非空、密码至少 6 位
/// 3. 哈希密码：let password_hash = auth::hash_password(&form.password)?;
/// 4. 是否管理员：let is_admin = form.is_admin.as_deref() == Some("1");
/// 5. 插入：
///    sqlx::query("INSERT INTO users (username, password_hash, is_admin) VALUES (?, ?, ?)")
///        .bind(&form.username).bind(&password_hash).bind(is_admin)
///        .execute(&state.pool).await.map_err(AppError::Database)?
/// 6. 重定向回管理页：Ok(Redirect::to("/admin/users"))
/// 返回类型说明：创建成功返回重定向（回到用户管理页）
pub async fn admin_create_user(
    State(state): State<AppState>,
    headers: HeaderMap,
    Form(form): Form<CreateUserForm>,
) -> Result<Redirect, AppError>
{
    // TODO(M1): 学生实现（步骤见上方注释）
    unimplemented!("M1 学生实现：创建用户")
}
