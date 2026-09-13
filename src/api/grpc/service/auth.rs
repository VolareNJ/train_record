// ============================================================
// api/grpc/service/auth.rs —— AuthService 实现（M9 第 6 步①）
// ============================================================
// 【教学：gRPC 登录与 REST 登录的流程差异】
//   REST  login：查用户 → 验密码 → 建 session → 响应头塞 Set-Cookie + JSON body
//   gRPC  login：查用户 → 验密码 → 建 session → **把 token 放进响应消息**
// 前四步一样（复用 auth::verify_password / auth::create_session），
// 只有"token 怎么交给客户端"不同：HTTP 靠 Cookie（浏览器自动保存），
// gRPC 靠响应消息体（客户端自己保存，下次放 metadata）。
//
// 【教学：为什么登录失败统一报"用户名不存在或密码错误"？】
// 两种失败（用户不存在 / 密码错）用**同一句**提示，避免"用户名枚举攻击"：
// 攻击者如果能看到"用户不存在"，就能一个个试出系统里有哪些账号。
// （REST 层 M8 也是这么做的，同一套安全纪律。）
//
// 【教学：与 REST 的状态码差异】
// REST 登录失败返回 400（Validation）；gRPC 这里用 UNAUTHENTICATED（16）——
// 语义更精确（"你给的身份凭证不对"），客户端也能据此直接弹登录框。
// 契约一旦发布就统一按 Code 判断，所以这里的选择会一直跟着接口走。
// ============================================================

use tonic::{Request, Response, Status};

use super::pool_of;
use crate::{
    AppState,
    api::grpc::{auth as guard, pb},
    auth as session,
};
// ⚠️ 挖空练习期间 login 还没实现，这个 import 暂时无用；
//    你实现 login 时会用它（查用户），实现完成后删掉这行 allow。
#[allow(unused_imports)]
use crate::models::User;

// ============================================================
// 【教学：service 实现体的组成】
// ============================================================
// 1) 一个只持有 AppState 的结构体（本文件的 AuthServiceImpl）
// 2) `#[tonic::async_trait]` + `impl pb::xxx_service_server::XxxService for ...`
//    （为什么需要 async_trait 属性：proto 生成的 trait 里是 `async fn`，
//     而 tonic 生成 trait 时用了 `#[async_trait]` 宏做跨线程包装，
//     实现方必须成对标注，否则签名不匹配。）
#[derive(Clone)]
pub struct AuthServiceImpl
{
    pub(crate) state: AppState,
}

impl AuthServiceImpl
{
    /// 构造函数：把共享状态塞进来（server.rs 里组装时调用）
    pub fn new(state: AppState) -> Self
    {
        Self { state }
    }
}

#[tonic::async_trait]
impl pb::auth_service_server::AuthService for AuthServiceImpl
{
    // ============================================================
    // Login（一元 RPC，★ 挖空练习）
    // ============================================================
    /// 登录：验证用户名密码 → 创建会话 → 返回用户信息 + token
    ///
    /// 【实现步骤】
    /// 1. let req = request.into_inner();
    ///    （Request<T> 是"信封"：metadata 在信封上，body 要用 into_inner 取出来）
    /// 2. let pool = pool_of(&self.state).await;
    /// 3. 查用户（本项目没有 `get_user_by_username` 这个现成函数，
    ///    直接查一次；这一处 SQL 与 REST 的 login 重复，已在 todo.md 记录）：
    ///      sqlx::query_as::<_, User>("SELECT * FROM users WHERE username = ?")
    ///          .bind(&req.username).fetch_optional(&pool).await
    ///          .map_err(|e| { tracing::error!("gRPC 登录查询失败: {e}");
    ///                         Status::internal("数据库错误") })?
    ///    → None → Err(Status::unauthenticated("用户名不存在或密码错误"))
    /// 4. 验密码（复用逻辑层）：
    ///      session::verify_password(&req.password, &user.password_hash)
    ///          .map_err(|_| Status::internal("密码验证失败"))?
    ///      → false → Err(Status::unauthenticated("用户名不存在或密码错误"))
    /// 5. 建会话（复用逻辑层，返回 token）：
    ///      session::create_session(&pool, user.id).await
    ///          .map_err(|_| Status::internal("创建会话失败"))?
    /// 6. 返回：Ok(Response::new(pb::LoginResponse {
    ///              user: pb::User::from(&user),   // ← 转 DTO（绝不带 password_hash）
    ///              token,
    ///          }))
    ///
    /// 【提示：为什么不用 `?` 直接转 ApiError？】
    /// 这里没有走 REST 层，所以手上是 sqlx::Error / AppError；
    /// 用 map_err 显式转成 Status，顺便决定"哪些错误对外说、哪些进日志"。
    /// （走 REST 层的那 30 个方法才是 `?` 自动转换 From<ApiError> for Status。）
    #[allow(unused_variables)]
    async fn login(
        &self,
        request: Request<pb::LoginRequest>,
    ) -> Result<Response<pb::LoginResponse>, Status>
    {
        // 【实现步骤】见上方注释
        todo!("M9 练习：Login 实现") // 【待实现】
    }

    // ============================================================
    // Logout（一元 RPC，已实现，读作参考）
    // ============================================================
    /// 登出：销毁服务端 session（token 立即失效）
    ///
    /// 【教学：为什么这里**不**要求登录？】
    /// 与 REST 的 logout 同款逻辑：没带 token 也算"登出成功"——
    /// 目的已经达到（客户端不该拿到 401 然后困惑"我到底登出没有"）。
    /// 判断口诀（M8 学过）：缺失 = 拒绝请求，还是"没活可干"？这里是后者。
    async fn logout(&self, request: Request<pb::LogoutRequest>)
    -> Result<Response<pb::Ack>, Status>
    {
        let pool = pool_of(&self.state).await;

        // 有 token 才销毁（温柔跳过）
        if let Some(token) = guard::token_from_metadata(&request)
        {
            session::destroy_session(&pool, &token)
                .await
                .map_err(|_| Status::internal("销毁会话失败"))?;
        }

        Ok(Response::new(pb::Ack { ok: true }))
    }

    // ============================================================
    // GetMe（一元 RPC，已实现）
    // ============================================================
    /// 当前登录用户（客户端启动时自检 token 是否有效）
    async fn get_me(&self, request: Request<pb::GetMeRequest>)
    -> Result<Response<pb::User>, Status>
    {
        // ① 守卫：没登录 / token 失效 → Err(UNAUTHENTICATED)，方法体根本不会继续执行
        let user = guard::require_user(&request, &self.state).await?;

        // ②③ 没有数据库操作，直接转消息返回
        Ok(Response::new(pb::User::from(&user)))
    }
}

// 【教学：上面出现过两个名字很像的 User】
//   models::User（领域模型，含 password_hash，只在服务器内部使用）
//   pb::User（契约消息，对外，只有安全字段）
// 两者之间**只有一处转换**（convert.rs 的 From<&User> for pb::User），
// 所以"忘记过滤密码哈希"的风险被限制在一个文件里——这就是分层的价值。
