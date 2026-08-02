// ============================================================
// auth.rs —— 认证核心模块
// ============================================================
// 【教学说明】
// 这个模块是 M1 认证的核心，管三件事：
//   1. 密码哈希（注册时存哈希，登录时比对哈希）
//   2. Session 的创建/验证/销毁（登录状态管理）
//   3. 通过 session 查出用户、判断权限
//
// 为什么需要独立成模块？
//   登录/登出/用户管理这几个 handler 都要用到这些能力，
//   放在一个模块里，大家共用，避免重复代码。
//
// 设计原则：本模块【不接触 HTTP】。
//   它只操作数据库和密码，返回 Result<_, AppError>。
//   HTTP 层（handlers/auth.rs）负责解析请求、设置 cookie、重定向。
//   这样职责清晰：auth.rs 管"逻辑"，handlers 管"请求响应"。
//
// 📌 阶段要求：M1 你来实现本文件的 4 个函数。
//   完整实现已备份在 docs/learning_path/M1_ref/auth_ref.rs，
//   实现完成后对照检查（不要提前看）。
// ============================================================

// 【教学：本文件用到的导入】
// 下面每个导入都会在实现时用上（骨架阶段 unused 警告是正常的）。
use argon2::{
    Argon2,           // 哈希算法本体
    PasswordHash,     // 解析存下来的哈希串（verify 用）
    PasswordHasher,   // trait：提供 hash_password 方法
    PasswordVerifier, // trait：提供 verify_password 方法
    password_hash::{SaltString, rand_core::OsRng},
    //   SaltString = 盐的类型
    //   OsRng      = 操作系统安全随机数生成器（生成随机盐用）
};
use sqlx::SqlitePool; // 数据库连接池（所有函数都接收它）

use crate::{error::AppError, models::User};
//   AppError = 统一错误类型
//   User     = 用户结构体（get_user_by_session 的返回类型）

// ============================================================
// 【教学：密码哈希】
// 为什么不能存明文密码？
//   数据库泄露 = 密码全泄露（很多人所有网站用同一个密码！）
// 什么是哈希？
//   把密码变成一串不可逆的"指纹"：同样输入 → 同样输出，但输出无法还原输入。
// 为什么用 argon2 而不是简单的哈希（如 MD5）？
//   argon2 是"慢哈希"：故意设计得很慢（加盐 + 多轮），
//   攻击者暴力破解一个密码要花很久，成本极高。
//   而 MD5/SHA 是"快哈希"，一秒能算几亿次，破解毫无成本。
//
// 注意：argon2 哈希【每次结果都不同】（因为随机盐）！
// 所以验证不能用"重新哈希再比较"，必须用 verify。
// ============================================================

/// 密码哈希：注册时用。把明文密码变成不可逆的哈希串
///
/// 【教学：SaltString + OsRng】
/// - 盐（salt）= 每次哈希加的随机调味料，让相同密码产生不同哈希
/// - OsRng = 操作系统提供的安全随机数生成器
/// - SaltString::generate(&mut OsRng) = 生成一个随机盐
///
/// 【实现步骤】
/// 1. 生成随机盐：SaltString::generate(&mut OsRng)
/// 2. 用 Argon2::default() 哈希：
///    Argon2::default().hash_password(plain.as_bytes(), &salt)
///    返回 Result<PasswordHash, Error>，用 .map_err 转成 AppError
/// 3. 取哈希字符串：.to_string()
pub fn hash_password(plain: &str) -> Result<String, AppError>
{
    // TODO(M1): 学生实现
    // 提示：
    //   let salt = SaltString::generate(&mut OsRng);
    //   let hash = Argon2::default()
    //       .hash_password(plain.as_bytes(), &salt)
    //       .map_err(|e| AppError::Other(format!("密码哈希失败: {e}")))?;
    //   Ok(hash.to_string())
    unimplemented!("M1 学生实现：密码哈希")
}

/// 校验密码：登录时用。返回 true = 密码正确
///
/// 【教学：为什么用 verify 而不是重新哈希比较？】
/// 因为哈希带随机盐，同一密码每次哈希结果都不同。
/// verify 是"用存下来的哈希串去验证输入的密码"，
/// 它会把盐从哈希串里提取出来，重新计算再比较。
/// 相当于"用原章验原件"，而不是"重新刻章对比"。
///
/// 【实现步骤】
/// 1. 解析存储的哈希串：PasswordHash::new(hash)，Err 转 AppError
/// 2. 验证：Argon2::default().verify_password(plain.as_bytes(), &parsed_hash)
///    返回 Result，用 .is_ok() 转成 bool
pub fn verify_password(plain: &str, hash: &str) -> Result<bool, AppError>
{
    // TODO(M1): 学生实现
    // 提示：
    //   let parsed_hash = PasswordHash::new(hash)
    //       .map_err(|e| AppError::Other(format!("密码哈希格式无效: {e}")))?;
    //   Ok(Argon2::default()
    //       .verify_password(plain.as_bytes(), &parsed_hash)
    //       .is_ok())
    unimplemented!("M1 学生实现：密码校验")
}

// ============================================================
// 【教学：Session（会话）】
// Session = 一次"登录状态"的记录。
// 登录成功后：
//   1. 生成一个随机 token（通行证编号）
//   2. 把 (token, user_id, 过期时间) 存进数据库 sessions 表
//   3. token 放进 cookie 发给浏览器
// 之后每次请求浏览器自动带 cookie，服务器拿 token 查 sessions 表，
// 就知道"这是哪个用户"。
//
// 为什么 token 用随机值（uuid）而不是自增 id？
//   自增 id 可被遍历：1, 2, 3... 攻击者能伪造任意用户的 session。
//   随机 uuid 不可预测，只能从服务器发的 cookie 里拿到。
//
// 为什么下面三个 session 函数都要带 &SqlitePool（数据库连接池）参数？
//   因为本项目把 session 记录存在【数据库】的 sessions 表里，而不是内存里。
//   pool 就是执行 SQL 的"入口"——所有读/写数据库的操作都要通过它：
//     - create_session      → INSERT（把通行证登记进登记簿）
//     - get_user_by_session → SELECT（拿通行证编号去查登记簿）
//     - destroy_session     → DELETE（登出时把通行证从登记簿撕掉）
//
//   为什么不用内存存 session？（想想内存方案的致命缺点）
//     1. 服务器一重启，内存全清空 → 所有人被登出，体验极差
//     2. 多实例部署（负载均衡跑好几台服务器）时，各台内存互不相通，
//        在 A 机登录，B 机不认识你的 session → 用户一会登录一会掉线
//     数据库方案：重启不丢、多实例共享，一份记录大家都能查。
//
//   & 是什么意思？
//     & 是引用（借用）。函数只是【借用】连接池去查一下库，
//     用完就还回去，不把它拿走、不独占。
//     这也解释了为什么参数是 &SqlitePool 而不是 SqlitePool。
//
//   反例印证规律：
//     hash_password / verify_password 是纯计算（不碰数据库），所以不带 pool。
//     规律：凡是要读/写数据库的函数，参数里必有 pool；纯计算的函数，不需要。
// ============================================================

/// 创建 session：登录成功后调用
///
/// 返回 token 字符串，调用方（handler）把它放进 cookie 发给浏览器
///
/// 【实现步骤】
/// 1. 生成随机 token：uuid::Uuid::new_v4().to_string()
/// 2. 过期时间 = 当前时间 + 30 天，格式化成 Rfc3339 字符串：
///    time::OffsetDateTime::now_utc() + time::Duration::days(30)
///    .format(&time::format_description::well_known::Rfc3339)
/// 3. INSERT INTO sessions (user_id, token, expires_at) VALUES (?, ?, ?)
/// 4. 返回 token
pub async fn create_session(pool: &SqlitePool, user_id: i64) -> Result<String, AppError>
{
    // TODO(M1): 学生实现
    // 提示：
    //   let token = uuid::Uuid::new_v4().to_string();
    //   let expires_at = time::OffsetDateTime::now_utc() + time::Duration::days(30);
    //   let expires_at_str = expires_at
    //       .format(&time::format_description::well_known::Rfc3339)
    //       .map_err(|e| AppError::Other(format!("时间格式化失败: {e}")))?;
    //   sqlx::query("INSERT INTO sessions (user_id, token, expires_at) VALUES (?, ?, ?)")
    //       .bind(user_id)
    //       .bind(&token)
    //       .bind(expires_at_str)
    //       .execute(pool)
    //       .await
    //       .map_err(AppError::Database)?;
    //   Ok(token)
    unimplemented!("M1 学生实现：创建 session")
}

/// 验证 session：凭 token 查出用户
///
/// 无效（不存在/已过期）返回 Err(AppError::Unauthorized)
///
/// 【实现步骤】
/// 1. 联表查询：JOIN sessions 和 users，按 token 找用户
///    SELECT u.* FROM users u JOIN sessions s ON s.user_id = u.id WHERE s.token = ?
/// 2. fetch_optional 拿到 Option<User>，查不到 → Err(Unauthorized)
///    Option 的 .ok_or_else(|| AppError::Unauthorized) 适配器
/// 3. 返回用户
pub async fn get_user_by_session(pool: &SqlitePool, token: &str) -> Result<User, AppError>
{
    // TODO(M1): 学生实现
    // 提示：
    //   let user: Option<User> = sqlx::query_as::<_, User>(
    //       "SELECT u.* FROM users u
    //        JOIN sessions s ON s.user_id = u.id
    //        WHERE s.token = ?",
    //   )
    //   .bind(token)
    //   .fetch_optional(pool)
    //   .await
    //   .map_err(AppError::Database)?;
    //
    //   let user = user.ok_or_else(|| AppError::Unauthorized)?;
    //   Ok(user)
    //   （注意：完整版应在 SQL 里加 expires_at > now 条件检查过期。
    //     这里为教学清晰先简化，以后在 M4+ 完善。）
    unimplemented!("M1 学生实现：凭 token 查用户")
}

/// 销毁 session：登出时调用
///
/// 【实现步骤】
/// DELETE FROM sessions WHERE token = ?
pub async fn destroy_session(pool: &SqlitePool, token: &str) -> Result<(), AppError>
{
    // TODO(M1): 学生实现
    // 提示：
    //   sqlx::query("DELETE FROM sessions WHERE token = ?")
    //       .bind(token)
    //       .execute(pool)
    //       .await
    //       .map_err(AppError::Database)?;
    //   Ok(())
    unimplemented!("M1 学生实现：销毁 session")
}
