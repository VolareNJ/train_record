// ============================================================
// api/mod.rs —— REST API 层模块入口（M8）
// ============================================================
// 【教学说明】
// 本项目从 M0 到 M7 都是"页面（SSR）"出口：handler 查库后拼 HTML 给人看。
// M8 加第二个出口：**API（JSON）**，给程序读（M9 的 iced 桌面客户端）。
//
// 同一个数据库、同一套业务逻辑，两种出口：
//   页面路由（/today、/exercises...）→ HTML，给人看（handlers/）
//   API 路由（/api/v1/...）          → JSON，给程序读（本模块）
//
// 为什么新开 api/ 模块而不是改造 handlers/？
//   页面 handler 的职责是"渲染 HTML"，返回 JSON 会破坏它。
//   M8 的做法是新增一组 API handler，与页面 handler 并列。
//   （这也符合"先复制，后抽取"策略：第一版直接写 SQL，
//     同一查询出现 2 次以上再抽公共函数。）
//
// 本文件职责：
//   1. 定义 ApiError（API 错误 → JSON，不是 302！）
//   2. 组装 api::router()（所有 /api/v1/... 路由）
//
// 📌 阶段要求：M8 你来实现本文件所有函数与 ApiError。
//   完整实现已备份在 docs/learning_path/M8_ref/，实现完成后对照检查。
// ============================================================

// ============================================================
// 【教学：为什么 API 层需要自己的错误类型 ApiError？】
// ============================================================
// M7 第 2 步把页面错误改成了 302 跳登录页（给人看，体验好）。
// 但 API 客户端（iced）不需要跳转——它要的是**状态码 + JSON 错误信息**：
//   {"error": "未登录"}
//
// 对比：
//   AppError::Unauthorized → 302 重定向到 /login（浏览器跟着走）
//   ApiError::Unauthorized → 401 + JSON（程序收到明确信号）
//
// 如果 API 复用 AppError：iced 收到 302 会默默跟随跳转去拿 /login 的 HTML，
// 然后 JSON 解析失败——拿到的是 HTML 不是 JSON。语义不同必须分开。
//
// 【教学：IntoResponse】
// axum 要求"任何能返回给浏览器的类型"实现 IntoResponse trait。
// 我们让 ApiError 实现它，handler 里 return Err(ApiError::NotFound(...))
// axum 就知道把错误转成 HTTP 响应返回给客户端。
// ============================================================
use axum::{
    Json, Router,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, patch, post},
};
use serde_json::json;

use crate::AppState;

// ============================================================
// 【教学：ApiError —— API 层统一错误类型】
// ============================================================
// 7 个变体，和 AppError 几乎一一对应，但 Unauthorized 语义不同：
//   AppError::Unauthorized   → 302 跳登录页（给人）
//   ApiError::Unauthorized   → 401 JSON（给程序）
//
// 变体对照：
//   Database(sqlx::Error)    → 500 数据库错误
//   Unauthorized             → 401 未登录
//   NotFound(String)         → 404 资源不存在
//   Validation(String)       → 400 参数不合法
//   Forbidden(String)        → 403 无权限
//   Other(String)            → 500 其他
//
// 【教学：为什么 Validation 是 400 而不是 422？】
// 页面层 AppError::Validation 用 422（Unprocessable Entity），
// 因为表单语义上"字段合法但内容不行"。
// API 层 M8 文档统一用 400（Bad Request）——参数不合法。
// iced 客户端只需要"4xx = 请求有问题"，具体 400/422 区别不大，
// 用 400 更简单，也是大多数公开 API 的惯例。
#[derive(Debug)]
pub enum ApiError
{
    /// 数据库错误 → 500
    Database(sqlx::Error),
    /// 未登录 → 401 JSON（不是 302！）
    /// ⚠️ 挖空练习期间加 allow（ApiAuthUser 挖空后暂无人构造），实现完成后可删
    #[allow(dead_code)]
    Unauthorized,
    /// 资源不存在 → 404
    NotFound(String),
    /// 参数不合法 → 400
    Validation(String),
    /// 无权限 → 403
    Forbidden(String),
    /// 其他 → 500
    Other(String),
}

impl IntoResponse for ApiError
{
    fn into_response(self) -> Response
    {
        // 【教学：match 拆解错误 → (状态码, 消息) 元组】
        // 状态码决定 HTTP 语义，消息决定 JSON body 内容。
        let (status, message) = match self
        {
            // 数据库错误：记录日志（方便排查），对外统一"数据库错误"
            ApiError::Database(e) =>
            {
                tracing::error!("API 数据库错误: {e}");
                (StatusCode::INTERNAL_SERVER_ERROR, "数据库错误".to_string())
            },
            // 未登录：明确告诉程序"你没登录"
            ApiError::Unauthorized => (StatusCode::UNAUTHORIZED, "未登录".to_string()),
            ApiError::NotFound(msg) => (StatusCode::NOT_FOUND, msg),
            ApiError::Validation(msg) => (StatusCode::BAD_REQUEST, msg),
            ApiError::Forbidden(msg) => (StatusCode::FORBIDDEN, msg),
            ApiError::Other(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg),
        };
        // 【教学：关键 —— 返回 JSON 而不是 302！】
        // (status, Json(...)) 元组也实现了 IntoResponse，
        // axum 自动把 Json 序列化成 body，status 设成状态码。
        (status, Json(json!({ "error": message }))).into_response()
    }
}

// ============================================================
// 【教学：api::router() —— 组装所有 API 路由】
// ============================================================
// main.rs 里 .merge(api::router()) 把整棵 API 路由树合并进主路由。
// merge 要求两边 Router 的 state 类型一致：都是 AppState。
//
// 【教学：为什么用 .route(...).route(...) 链式，而不嵌套 Router？】
// 一个文件集中声明所有 API 路由（mod.rs），一眼看到全部端点。
// 每个子模块（auth/phases/exercises/...）只提供 handler 函数，
// 路由关系集中在这一个地方，方便对照 M8.md 的端点表。
//
// 【实现步骤】
// 1. 声明子模块（本文件已列好）
// 2. 用 Router::new() 链式注册所有 /api/v1/... 路由
// 3. 返回 Router<AppState>（与 main.rs 的 Router 状态一致）
pub mod auth;
pub mod exercises;
pub mod phases;
pub mod plans;
pub mod records;
pub mod stats;

pub fn router() -> Router<AppState>
{
    Router::new()
        // ----------------------------------------------------------
        // 第 2 步：认证 API（src/api/auth.rs）
        // ----------------------------------------------------------
        .route("/api/v1/login", post(auth::login))
        .route("/api/v1/logout", post(auth::logout))
        .route("/api/v1/me", get(auth::me))
        // ----------------------------------------------------------
        // 第 3 步：阶段 API（src/api/phases.rs）
        // ----------------------------------------------------------
        .route("/api/v1/phases", get(phases::list).post(phases::create))
        .route(
            "/api/v1/phases/{id}",
            get(phases::detail).patch(phases::update),
        )
        .route("/api/v1/phases/{id}/archive", post(phases::archive))
        .route("/api/v1/phases/{id}/unarchive", post(phases::unarchive))
        // ----------------------------------------------------------
        // 第 4 步：动作库 API（src/api/exercises.rs）
        // ----------------------------------------------------------
        .route(
            "/api/v1/exercises",
            get(exercises::list).post(exercises::create),
        )
        .route(
            "/api/v1/exercises/{id}",
            get(exercises::detail)
                .patch(exercises::update)
                .delete(exercises::delete),
        )
        // ----------------------------------------------------------
        // 第 5 步：模板 + 计划 API（src/api/plans.rs）
        // ----------------------------------------------------------
        .route(
            "/api/v1/phases/{phase_id}/templates",
            get(plans::template_list).post(plans::template_create),
        )
        .route(
            "/api/v1/templates/{id}",
            patch(plans::template_update).delete(plans::template_delete),
        )
        .route(
            "/api/v1/phases/{phase_id}/plans",
            get(plans::plan_list).post(plans::plan_create),
        )
        .route(
            "/api/v1/plans/{id}",
            get(plans::plan_detail)
                .patch(plans::plan_update)
                .delete(plans::plan_delete),
        )
        // ----------------------------------------------------------
        // 第 6 步：记录 API（src/api/records.rs）
        // ----------------------------------------------------------
        .route("/api/v1/today", get(records::today))
        .route(
            "/api/v1/plans/{plan_id}/items/{item_id}/records",
            post(records::upsert_record),
        )
        .route("/api/v1/records", get(records::list_by_date))
        .route(
            "/api/v1/records/{id}",
            patch(records::update_record).delete(records::delete_record),
        )
        // ----------------------------------------------------------
        // 第 7 步：统计 API（src/api/stats.rs）
        // ----------------------------------------------------------
        .route("/api/v1/history", get(stats::calendar))
        .route("/api/v1/history/{date}", get(stats::history_day))
        .route("/api/v1/exercises/{id}/stats", get(stats::exercise_stats))
}
