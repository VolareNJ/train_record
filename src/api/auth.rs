// ============================================================
// api/auth.rs —— 认证 API（M8 第 2 步）
// ============================================================
// 【教学说明】
// 页面层已经有登录/登出（handlers/auth.rs），
// API 层为什么还要一份？因为返回格式不同：
//   页面 login  → 302 重定向 + Set-Cookie（浏览器自动跟随）
//   API login   → 200 + JSON 用户信息 + Set-Cookie（程序读 JSON）
//
// 复用什么？
//   auth::verify_password（验证密码）—— 逻辑层已有
//   auth::create_session（创建 session）—— 逻辑层已有
//   只是"表现层"（HTTP 请求/响应格式）换成 JSON。
//
// 端点：
//   POST /api/v1/login   登录 → {"user": {...}} + Set-Cookie
//   POST /api/v1/logout  登出 → 销毁 session
//   GET  /api/v1/me      当前用户（API 认证自检）
//
// 📌 阶段要求：M8 你来实现本文件所有函数。
//   完整实现已备份在 docs/learning_path/M8_ref/，实现完成后对照检查。
// ============================================================

// ============================================================
// 【教学：DTO（Data Transfer Object）—— 为什么不能直接序列化 User】
// ============================================================
// models::User 有 password_hash（argon2 密码哈希）字段，
// 它是数据库列的镜像，绝不能暴露给 API 调用者！
// 所以 API 输出用专门的 DTO 结构体 UserOut，只挑安全的字段。
// 用 From<&User> 实现转换，serde 只序列化 UserOut（安全字段）。
// ============================================================
use axum::{
    Json,
    extract::State,
    http::{HeaderMap, header::SET_COOKIE, request::Parts},
};
use serde::{Deserialize, Serialize};

use crate::{AppState, api::ApiError, auth, handlers::auth::extract_token, models::User};

// ============================================================
// 【教学：ApiAuthUser —— API 版登录守卫】
// ============================================================
// 页面层的 AuthUser 提取器 Rejection = AppError：
//   未登录 → AppError::Unauthorized → 302 跳登录页
// API 层要 401 JSON，所以复制一份提取逻辑，Rejection = ApiError。
//
// 【教学：为什么是"复制"而不是"复用"？】
// 两个提取器的区别只有 Rejection 类型不同（AppError vs ApiError）。
// 泛型化 AuthUser 可以消除复制（M9 再考虑），M8 先复制：
//   ① 改动最小，不碰页面层已测过的代码
//   ② 教学上直观——"同一个守卫，换一个错误出口"
//   ③ M8 文档明确"先复制，后抽取"，M9 iced 开始前再统一
//
// 【教学：extract_token 为什么从 handlers::auth 导入？】
// extract_token 是"从 Cookie 头解析 session token"的纯函数，
// 页面层和 API 层都要用。它原本是 handlers/auth.rs 的私有函数，
// M8 把它改成 pub（只改可见性，逻辑不动）——单一事实来源。
// ============================================================
/// API 已登录用户（M8 守卫提取器，Rejection = ApiError）
///
/// 用法：handler 签名里写 `ApiAuthUser(user): ApiAuthUser`，
/// 未登录请求会在调用 handler 前被拦截（401 JSON）。
pub struct ApiAuthUser(pub User);

impl axum::extract::FromRequestParts<AppState> for ApiAuthUser
{
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection>
    {
        // 【实现步骤】
        // 1. let pool = state.pool.read().await.clone();
        // 2. let token = extract_token(&parts.headers).ok_or(ApiError::Unauthorized)?;
        // 3. let user = auth::get_user_by_session(&pool, &token).await
        //      .map_err(|_| ApiError::Unauthorized)?;
        //    （session 失效/过期也是"未登录"→ 统一 401，
        //     不把内部错误细节暴露给 API 调用者）
        // 4. Ok(ApiAuthUser(user))
        //
        // 提示：auth::get_user_by_session 返回 Result<User, AppError>，
        //   AppError 不能直接 ? 转成 ApiError（没有 From 实现），
        //   需要 map_err 转成 ApiError::Unauthorized。
        let pool = state.pool.read().await.clone();
        let token = extract_token(&parts.headers).ok_or(ApiError::Unauthorized)?;
        let user = auth::get_user_by_session(&pool, &token)
            .await
            .map_err(|_| ApiError::Unauthorized)?;
        Ok(ApiAuthUser(user))
    }
}

// ============================================================
// 【教学：LoginReq —— API 登录请求体】
// ============================================================
// 页面层用 Form<LoginForm>（application/x-www-form-urlencoded），
// API 层用 Json<LoginReq>（application/json）——程序客户端传 JSON。
// serde 的 Deserialize 让结构体可以从 JSON body 自动填充：
//   {"username": "admin", "password": "admin123"}
#[derive(Deserialize)]
pub struct LoginReq
{
    pub username: String,
    pub password: String,
}

// ============================================================
// 【教学：UserOut —— 用户信息 DTO（安全输出）】
// ============================================================
// 只暴露 id/username/is_admin/body_weight，绝不暴露 password_hash。
// From<&User> 实现转换：let out = UserOut::from(&user);
#[derive(Serialize)]
pub struct UserOut
{
    pub id: i64,
    pub username: String,
    pub is_admin: bool,
    pub body_weight: Option<f64>,
}

impl From<&User> for UserOut
{
    fn from(u: &User) -> Self
    {
        Self {
            id: u.id,
            username: u.username.clone(),
            is_admin: u.is_admin,
            body_weight: u.body_weight,
        }
    }
}

// ============================================================
// 登录（POST /api/v1/login）
// ============================================================
/// API 登录：验证用户名密码 → 创建 session → 返回 JSON 用户信息 + Set-Cookie
///
/// 【教学：与页面 login 的流程对比】
///   页面 login：查用户 → 验证密码 → 建 session → 拼 cookie → 302 重定向
///   API login ：查用户 → 验证密码 → 建 session → 拼 cookie → 返回 JSON
///   前 5 步完全一样（复用逻辑层函数），只有最后一步出口不同。
///
/// 【教学：响应结构 —— 返回头 + Json 的元组】
/// 页面 login 返回 (HeaderMap, Redirect)，API 返回 (HeaderMap, Json)。
/// 元组实现 IntoResponse：HeaderMap 进响应头（Set-Cookie），Json 进 body。
/// 客户端同时拿到：Set-Cookie（浏览器自动存 cookie）+ JSON（用户信息 + token）。
///
/// 【教学：为什么 body 要带 token？】
/// 浏览器客户端（调试 API）靠 Set-Cookie 自动带 session，
/// 但 iced 等非浏览器客户端不会自动存 cookie——它需要手动保存 token，
/// 每次请求自己带 Cookie 头。所以 body 里冗余返回一份 token（M8.md §2.4）。
/// （Authorization: Bearer 头认证留到 M9 做，见 todo.md 扩展点。）
///
/// 【实现步骤】
/// 1. 签名：State + Json<LoginReq>
/// 2. 查用户：SELECT * FROM users WHERE username = ?
///    → fetch_optional → None → Err(Validation("用户名不存在或密码错误"))
/// 3. 验证密码：auth::verify_password(&req.password, &user.password_hash)?
///    （返回 Result<bool, AppError>，false → 同上错误）
/// 4. 创建 session：auth::create_session(&pool, user.id).await?
///    （返回 Result<String, AppError>——注意转 ApiError）
/// 5. 拼 cookie：format!("session={}; Path=/; HttpOnly; SameSite=Lax; Max-Age=2592000")
///    （Max-Age=2592000 秒 = 30 天，与页面一致）
/// 6. 建 HeaderMap，插入 SET_COOKIE
/// 7. 返回 (headers, Json(json!({"user": ..., "token": token})))
///
/// 【教学：AppError → ApiError 的转换】
/// verify_password / create_session 返回 Result<_, AppError>，
/// 本文件错误类型是 ApiError。两种转换方式：
///   a. map_err 逐个转（直观）：.map_err(|_| ApiError::Other(...))
///   b. 实现 From<AppError> for ApiError（全局一次转换）
/// M8 先用法 a（教学直观），出现多次重复后再抽 From 实现。
pub async fn login(
    State(state): State<AppState>,
    Json(req): Json<LoginReq>,
) -> Result<(HeaderMap, Json<serde_json::Value>), ApiError>
{
    let pool = state.pool.read().await.clone();

    // 1. 查用户（按用户名，用户表没有 user_id 概念）
    let user_op = sqlx::query_as::<_, User>("SELECT * FROM users WHERE username = ?")
        .bind(&req.username)
        .fetch_optional(&pool)
        .await
        .map_err(ApiError::Database)?;

    let user = user_op.ok_or_else(|| ApiError::Validation("用户名不存在或密码错误".to_string()))?;

    // 2. 验证密码（逻辑层复用）
    let is_correct = auth::verify_password(&req.password, &user.password_hash)
        .map_err(|_| ApiError::Other("密码验证失败".to_string()))?;
    if !is_correct
    {
        return Err(ApiError::Validation("用户名不存在或密码错误".to_string()));
    }

    // 3. 创建 session（逻辑层复用）
    let token = auth::create_session(&pool, user.id)
        .await
        .map_err(|_| ApiError::Other("创建会话失败".to_string()))?;

    // 4. 拼 cookie + 响应头
    let cookie = format!(
        "session={}; Path=/; HttpOnly; SameSite=Lax; Max-Age=2592000",
        token
    );
    let mut headers = HeaderMap::new();
    headers.insert(
        SET_COOKIE,
        cookie
            .parse()
            .map_err(|_| ApiError::Other("头文件生成失败".to_string()))?,
    );

    // 5. 返回用户信息 + token（M8.md §2.4：token 给非浏览器客户端）
    Ok((
        headers,
        Json(serde_json::json!({
            "user": UserOut::from(&user),
            "token": token,
        })),
    ))
}

// ============================================================
// 登出（POST /api/v1/logout）
// ============================================================
/// API 登出：销毁 session，清除 cookie，返回 {"ok": true}
///
/// 【教学：与页面 logout 的差异】
/// 页面 logout：销毁 session → 302 回登录页（浏览器跟随）
/// API logout ：销毁 session → 200 + {"ok": true}（程序确认）
///
/// 【教学：extract_token 返回 Option —— 没 token 怎么办？】
/// 和页面层同款逻辑：没有 token = "没活可干"（不强制报错）。
/// 用 if let Some(token) 温柔跳过，统一返回成功。
/// （为什么不是 ? 强制？登出操作"没登录也能登出"是安全行为，
///   返回错误反而让客户端困惑。判断口诀：缺失 = 拒绝请求 还是 没活可干？）
///
/// 【实现步骤】
/// 1. 签名：State + HeaderMap（读 Cookie 头，不需要守卫——没登录也能登出）
/// 2. if let Some(token) = extract_token(&headers) { auth::destroy_session(...) }
/// 3. 清除 cookie：Set-Cookie: session=; Max-Age=0（同页面逻辑）
/// 4. 返回 Json(json!({"ok": true}))
pub async fn logout(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<(HeaderMap, Json<serde_json::Value>), ApiError>
{
    let pool = state.pool.read().await.clone();

    // 有 token 才销毁（温柔跳过，见上方教学）
    if let Some(token) = extract_token(&headers)
    {
        auth::destroy_session(&pool, &token)
            .await
            .map_err(|_| ApiError::Other("销毁会话失败".to_string()))?;
    }

    // 清除浏览器 cookie（Max-Age=0 立即过期）
    let mut resp_headers = HeaderMap::new();
    resp_headers.insert(
        SET_COOKIE,
        "session=; Path=/; HttpOnly; SameSite=Lax; Max-Age=0"
            .parse()
            .map_err(|_| ApiError::Other("头文件生成失败".to_string()))?,
    );

    Ok((resp_headers, Json(serde_json::json!({ "ok": true }))))
}

// ============================================================
// 当前用户（GET /api/v1/me）
// ============================================================
/// API 认证自检：返回当前登录用户信息（未登录 → 401 JSON）
///
/// 【教学：ApiAuthUser 的作用在这里体现】
/// handler 签名里写 ApiAuthUser(user)，axum 在调用前自动验证：
///   有合法 session → 提取 User → 调用本函数
///   无/无效 session → 401 JSON（根本进不来）
/// 这就是"声明式守卫"——handler 里不用写任何验证代码。
///
/// 【实现步骤】
/// 1. 签名：State + ApiAuthUser(user)
/// 2. 返回 Json(UserOut::from(&user))
pub async fn me(
    State(_state): State<AppState>,
    ApiAuthUser(user): ApiAuthUser,
) -> Result<Json<UserOut>, ApiError>
{
    Ok(Json(UserOut::from(&user)))
}
