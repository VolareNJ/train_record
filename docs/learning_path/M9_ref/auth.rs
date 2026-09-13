// ============================================================
// api/grpc/auth.rs —— gRPC 版登录守卫（M9 第 4 步）
// ============================================================
// 【教学：REST 的守卫 vs gRPC 的守卫】
// M8（REST）用 axum 提取器 `ApiAuthUser`：请求进 handler 之前自动验 cookie，
//   handler 签名里写 `ApiAuthUser(user)` 就等于"声明这里要求登录"。
// gRPC 没有"提取器"这一层可插入点（tonic 的 trait 方法是编译器生成的），
// 所以改用**显式函数调用**：
//     let user = auth::require_user(&request, &self.state).await?;
// 每个方法的第一行就是它。
//
// 【教学：为什么不用 tonic 拦截器（Interceptor）做统一守卫？】
// tonic 有 `Interceptor`（等价于 axum 的 middleware），能用 `Server::builder()
// .layer(InterceptorLayer::new(...))` 全局拦截。但有个现实问题：
//   拦截器只能拿到"请求元信息"，要查数据库/拿 AppState 得把状态搬进去，
//   而"哪些方法需要登录"（Login 不需要、GetMe 需要）也要在拦截器里
//   再判一遍方法名 —— 逻辑会散成两处。
// 结论：**个人项目里"显式一行守卫"更好读**：谁需要登录，一眼看到。
// （要点：这不是"gRPC 不支持中间件"，而是"这次选择更简单的方案"。
//   将来方法变多/权限变复杂，再换成 Interceptor + 方法白名单。）
//
// 【教学：token 放在哪？—— gRPC 的 metadata】
// gRPC 没有 Cookie 概念，但 HTTP/2 有 headers，gRPC 把它叫 **metadata**
// （键值对，键必须小写 ASCII）。惯例：
//     authorization: Bearer <token>     ← 与 HTTP 生态通用（推荐）
// 本项目额外兼容一个更好手打的：
//     x-session-token: <token>
// 客户端（iced / grpcurl）二选一即可。
//
// ⚠️ 关于"明文传输"：metadata 是明文（无 TLS 时）。生产暴露到公网时
//    应该套 TLS（tonic 的 tls 特性），否则 token 会被中间人看到——
//    当前部署在本机/内网，与 web 版同一信任边界，M9 不做 TLS（见 todo.md）。
// ============================================================

use crate::{AppState, auth, models::User};
use tonic::{Request, Status};

/// 从请求 metadata 里取 token
///
/// 【教学：为什么这是普通函数而不是 async？】
/// 它只读内存里的 metadata（不发网络、不查库），没有 .await 的地方。
/// Rust 里"能写成同步就写成同步"是好习惯：可以被任何地方调用
/// （包括不支持 async 的上下文），编译也更简单。
///
/// 【教学：为什么是 pub(crate)？】
/// require_user 用它，Logout 也要用它（登出需要 token 才能销毁 session）。
/// 可见性只开到"本 crate 内"——不给外部用，就不算公开 API。
///
/// 【教学：metadata 的取值 API】
///   request.metadata()            → &MetadataMap
///   .get("authorization")         → Option<&MetadataValue<Ascii>>
///   .and_then(|v| v.to_str().ok())→ Option<&str>
///     （to_str() 会因为非法字节返回 Err；metadata 里可能塞任意二进制，
///       所以必须容错——这一步不能 unwrap）
pub(crate) fn token_from_metadata<T>(request: &Request<T>) -> Option<String>
{
    let metadata = request.metadata();

    // 优先取标准头：authorization: Bearer <token>
    if let Some(value) = metadata.get("authorization").and_then(|v| v.to_str().ok())
    {
        // strip_prefix 返回 Option<&str>：没有 "Bearer " 前缀就是 None
        // （不区分大小写会让实现变复杂，这里按规范要求客户端写标准写法）
        return value
            .strip_prefix("Bearer ")
            .map(|token| token.trim().to_string());
    }

    // 兼容手打调试：x-session-token: <token>
    metadata
        .get("x-session-token")
        .and_then(|v| v.to_str().ok())
        .map(|token| token.trim().to_string())
}

// ============================================================
// 【教学：require_user —— "gRPC 版 AuthUser 提取器"】
// ============================================================
// 输入：一个泛型请求（不关心具体是什么消息）+ 应用状态
// 输出：已登录用户 User，或一个 Status 错误
//
// 【教学：为什么要泛型 <T>？】
// 每个 RPC 的请求消息类型都不同（Request<ListPhasesRequest>、
// Request<GetPhaseRequest>…），但我们只关心 metadata（在请求的"壳"上），
// 不关心 body。泛型 T 表示"任意消息类型"，于是 6 个 service 共用这一个函数。
// 对比 REST：提取器的 RequestParts 相当于这里的泛型壳。
//
// 【实现步骤】
// 1. let token = token_from_metadata(request)
//      .ok_or_else(|| Status::unauthenticated("缺少 token：请在 metadata 里带
//                     authorization: Bearer <token>"))?;
//    （⚠️ 这里用 Status::unauthenticated 直接构造，不走 ApiError —— 因为
//      "没带 token"是协议层问题，还没进到业务层）
// 2. let pool = state.pool.read().await.clone();
// 3. let user = auth::get_user_by_session(&pool, &token).await
//      .map_err(|_| Status::unauthenticated("会话无效或已过期"))?;
//    （⚠️ get_user_by_session 返回 Result<User, AppError>；这里把**所有**错误
//      都归为"未登录"——与 REST 守卫同样的选择：不把内部错误细节暴露给客户端）
// 4. Ok(user)
//
// 【教学：为什么签名是 &Request<T> 而不是 Request<T>？】
// 取 metadata 不需要消费请求——后面 handler 还要 request.into_inner() 拿 body。
// 借用（&）不夺走所有权，调用方先守卫、再取 body，顺序自然。
/// 【实现步骤】见上方注释
pub(crate) async fn require_user<T>(request: &Request<T>, state: &AppState)
-> Result<User, Status>
{
    let token = token_from_metadata(request).ok_or_else(|| {
        Status::unauthenticated("缺少 token：请在 metadata 里带 authorization: Bearer <token>")
    })?;

    let pool = state.pool.read().await.clone();
    let user = auth::get_user_by_session(&pool, &token)
        .await
        .map_err(|_| Status::unauthenticated("会话无效或已过期"))?;

    Ok(user)
}
