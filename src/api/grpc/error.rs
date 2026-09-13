// ============================================================
// api/grpc/error.rs —— 错误映射：ApiError → tonic::Status（M9 第 3 步）
// ============================================================
// 【教学：为什么错误要"翻译"一次？】
// M8 的 ApiError 是 **HTTP 语义**的错误（401/403/404/400/500 + JSON body）。
// gRPC 的错误模型完全不同：
//   · 错误不放在 body 里，而是放在 trailer（HTTP/2 的尾部头）
//   · 状态码不是数字 401，而是枚举 Code::Unauthenticated（10/16 编号见下）
//   · 客户端拿到的是 `tonic::Status`（code + message + 可选 details）
//
// 两个模型对照（记这一张表就够）：
//   HTTP             gRPC Code              gRPC 数字编号
//   401 Unauthorized UNAUTHENTICATED        16   ← "我没登录/凭证无效"
//   403 Forbidden    PERMISSION_DENIED      7    ← "登录了但没权限"
//   404 Not Found    NOT_FOUND              5
//   400 Bad Request  INVALID_ARGUMENT       3
//   409 Conflict     ALREADY_EXISTS         6
//   500 Server Error INTERNAL               13
//   （200 OK          OK                     0    ← 成功不是错误，走正常 Response）
//
// 【教学：为什么做成 `From` trait，而不是每个 service 里手写 match？】
// trait 实现是**一处定义、全项目可用**：
//   · service 里写 `rest::phases::phase_list(...).await.map_err(Status::from)?`
//     或更短的 `.map_err(Status::from)?`（等价于 `?` 的自动转换，如果实现了 From）
//   · 甚至直接 `...?`（因为 `?` 会自动调用 From::from 把 ApiError 变成 Status！）
// 这是 Rust 错误处理的核心机制：**`?` 遇到类型不匹配时，自动尝试 From 转换**。
// 所以只要这里有 impl From<ApiError> for Status，service 里就能一路 `?` 到底。
//
//  注意：`?` 的自动转换只在"函数返回 Result<_, Status>"时生效——
// 而 tonic 生成的方法正是返回 Result<_, Status>，所以完美契合。
// ============================================================

// ============================================================
// 【教学：Message 该给客户端看什么？】
// 原则：**客户端能据此决定行为的，才给它**。
//   · 字段校验（"重量不能为负"）→ 给（用户要照着改）
//   · 资源不存在（"计划不存在"）→ 给（客户端要刷新列表）
//   · 数据库错误细节（SQL 语句、表名）→ 不给（内部信息，可能被用来探测系统）
//     统一换成"数据库错误"，细节写进服务器日志（tracing::error!）——
//     与 M8 的 ApiError::Database 处理方式完全一致。
// ============================================================
impl From<crate::api::rest::ApiError> for tonic::Status
{
    // 挖空练习期间加 allow 消除 unused 警告，实现完成后可删
    #[allow(unused)]
    fn from(err: crate::api::rest::ApiError) -> Self
    {
        // 【实现步骤】
        // 1. match err，把 6 个变体各自映射成 (Code, message)：
        //      ApiError::Database(e)   → 先 tracing::error!("gRPC 数据库错误: {e}")
        //                                再 (Code::Internal, "数据库错误")
        //      ApiError::Unauthorized  → (Code::Unauthenticated, "未登录")
        //      ApiError::NotFound(msg) → (Code::NotFound, msg)
        //      ApiError::Validation(msg)   → (Code::InvalidArgument, msg)
        //      ApiError::Forbidden(msg)    → (Code::PermissionDenied, msg)
        //      ApiError::Other(msg)        → (Code::Internal, msg)
        // 2. 用 Status::new(code, message) 构造返回
        //
        // 【提示：为什么用 match 而不是 if/else 链？】
        //   枚举 + match 让编译器保证"每个变体都被处理过"——
        //   将来给 ApiError 加第 7 个变体时，这里会**编译报错**提醒你补映射
        //   （如果漏了，客户端会拿到一个语义不清的状态码）。
        //
        // 【提示：Database(e) 里的 e 要"用掉"】
        //   把它写进日志就够（`tracing::error!` 用了 e，编译器不会再报 unused）。
        todo!("M9 练习：ApiError → tonic::Status 映射实现") // 【待实现】
    }
}
