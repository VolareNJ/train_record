// ============================================================
// error.rs —— 统一错误类型模块
// ============================================================
// 【教学说明】
// Rust 的错误处理是它最著名的特性之一。
// 初学者最大的困惑是："为什么每个函数都要写 Result<..., ...>？"
//
// 简单理解：
//   1. Rust 没有异常（try/catch），错误是"值"，通过返回值传递
//   2. 函数返回 Result<Ok类型, Err类型>，调用方必须处理错误
//   3. 但我们不想每个函数都写一堆错误类型，于是有了这个模块：
//      【定义一个统一的 AppError】，所有可能出错的地方都转成它
//
// 本项目 AppError 要能表示三类错误：
//   - 数据库错误（sqlx）
//   - 模板渲染错误（askama）
//   - 其他运行时错误
// 用 Rust 的 enum + #[from] 自动转换实现。
// ============================================================

use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Redirect, Response},
};
use serde_json::json;

/// 应用统一错误类型
///
/// 【教学说明】
/// - enum（枚举）：列举所有可能的错误变体
/// - #[derive(Debug)]：让错误可以被打印（println! 需要 Debug）
/// - #[derive(thiserror::Error)] 需要引入 thiserror crate，这里暂时手写
/// - 每个变体存一个底层错误，用 Box 包裹避免递归大小问题
#[derive(Debug)]
pub enum AppError
{
    /// 数据库操作失败（sqlx::Error）
    Database(sqlx::Error),
    /// 模板渲染失败（askama::Error）
    Template(askama::Error),
    /// 未登录或会话无效
    Unauthorized,
    /// 请求的数据不存在（如找不到某个训练记录）
    NotFound(String),
    /// 参数校验失败（如体重是负数）
    Validation(String),
    /// 拒绝
    Forbidden(String),
    /// 其他未分类错误
    Other(String),
}

// ============================================================
// 【教学：IntoResponse】
// Axum 框架要求"任何能返回给浏览器的类型"都实现 IntoResponse trait。
// 我们让 AppError 实现它，这样：
//   - handler 里 return Err(AppError::NotFound(...))
//   - axum 就知道把错误转成 HTTP 响应返回给浏览器
// ============================================================
impl IntoResponse for AppError
{
    /// 把 AppError 转成 HTTP 响应
    fn into_response(self) -> Response
    {
        // 根据错误类型决定 HTTP 状态码
        let (status, message) = match self
        {
            AppError::Database(e) =>
            {
                // 数据库错误：服务器内部错误 500
                // 【教学】tracing::error! 记录日志，方便排查
                tracing::error!("数据库错误: {e}");
                (StatusCode::INTERNAL_SERVER_ERROR, "数据库错误".to_string())
            },
            AppError::Template(e) =>
            {
                tracing::error!("模板渲染错误: {e}");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "页面渲染错误".to_string(),
                )
            },
            AppError::Unauthorized =>
            {
                // M7 第 2 步：未登录访问页面 → 302 跳转登录页（给"人"看）
                // ⚠️ M8 的 REST API 会用自己的 ApiError 返回 401 JSON，
                //    这里只管页面，全局跳转即可。
                // Redirect 本身实现了 IntoResponse，直接 .into_response()
                return Redirect::to("/login").into_response();
            },
            AppError::NotFound(msg) =>
            {
                // 404：找不到
                (StatusCode::NOT_FOUND, msg)
            },
            AppError::Validation(msg) =>
            {
                // 422：参数不合法
                (StatusCode::UNPROCESSABLE_ENTITY, msg)
            },
            AppError::Forbidden(msg) =>
            {
                // 403：拒绝
                (StatusCode::FORBIDDEN, msg)
            },
            AppError::Other(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg),
        };

        // 返回 JSON 格式的错误信息
        // 【教学】json! 宏快速构造 JSON；Response::builder() 组装响应
        let body = Json(json!({
            "error": message,
        }));

        // 把 JSON 和状态码打包成最终响应
        (status, body).into_response()
    }
}

// ============================================================
// 【教学：From 转换】
// Rust 里 `?` 运算符有个魔法：如果函数返回 Err(A)，
// 而函数签名要求 Err(B)，只要 A 实现了 From<A> for B，
// `?` 就会自动把 A 转成 B。
//
// 所以我们给 sqlx::Error 和 askama::Error 实现 From，
// 之后在 handler 里写 `sqlx::query!(...).fetch_one(&pool).await?`，
// 出错时会自动变成 AppError::Database(...)，代码非常干净。
// ============================================================

/// sqlx 错误自动转成 AppError
impl From<sqlx::Error> for AppError
{
    fn from(e: sqlx::Error) -> Self
    {
        AppError::Database(e)
    }
}

/// askama 模板错误自动转成 AppError
impl From<askama::Error> for AppError
{
    fn from(e: askama::Error) -> Self
    {
        AppError::Template(e)
    }
}

/// 字符串错误自动转成 AppError::Other
impl From<String> for AppError
{
    fn from(e: String) -> Self
    {
        AppError::Other(e)
    }
}

/// 自定义 Result 别名，省得每个函数写全名
///
/// 【教学说明】
/// type 别名：Result<T, AppError> 太长了，
/// 写成 AppResult<T> 让签名更简洁。
/// 用法：`pub async fn foo() -> AppResult<()>`
pub type AppResult<T> = Result<T, AppError>;
