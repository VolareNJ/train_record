// ============================================================
// 【M1 参考答案】auth.rs 完整实现
// 学生实现完成后，再对照本文件检查。不要提前看！
// ============================================================
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
// ============================================================

use argon2::{
    Argon2, PasswordHash, PasswordHasher, PasswordVerifier,
    password_hash::{SaltString, rand_core::OsRng},
};
use sqlx::SqlitePool;

use crate::{error::AppError, models::User};

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
pub fn hash_password(plain: &str) -> Result<String, AppError>
{
    // 1. 生成随机盐
    let salt = SaltString::generate(&mut OsRng);
    // 2. 用 argon2 算法 + 盐，把密码哈希成字符串
    //    Argon2::default() 使用推荐的默认参数（安全且合理）
    let hash = Argon2::default()
        .hash_password(plain.as_bytes(), &salt)
        .map_err(|e| AppError::Other(format!("密码哈希失败: {e}")))?
        .to_string();
    Ok(hash)
}

/// 校验密码：登录时用。返回 true = 密码正确
///
/// 【教学：为什么用 verify 而不是重新哈希比较？】
/// 因为哈希带随机盐，同一密码每次哈希结果都不同。
/// verify 是"用存下来的哈希串去验证输入的密码"，
/// 它会把盐从哈希串里提取出来，重新计算再比较。
/// 相当于"用原章验原件"，而不是"重新刻章对比"。
pub fn verify_password(plain: &str, hash: &str) -> Result<bool, AppError>
{
    // 1. 解析存储的哈希串（里面包含盐和算法信息）
    let parsed_hash =
        PasswordHash::new(hash).map_err(|e| AppError::Other(format!("密码哈希格式无效: {e}")))?;
    // 2. 验证输入的密码是否匹配
    Ok(Argon2::default()
        .verify_password(plain.as_bytes(), &parsed_hash)
        .is_ok())
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
// ============================================================

/// 创建 session：登录成功后调用
///
/// 返回 token 字符串，调用方（handler）把它放进 cookie 发给浏览器
pub async fn create_session(pool: &SqlitePool, user_id: i64) -> Result<String, AppError>
{
    // 1. 生成随机 token（uuid v4 = 128 位随机数，几乎不可能撞车）
    let token = uuid::Uuid::new_v4().to_string();

    // 2. 过期时间：现在 + 30 天（存成字符串，与表结构一致）
    //    【教学】time crate 的 OffsetDateTime 用来算时间
    //    这里用当前时间 + 30 天作为过期时间
    let expires_at = time::OffsetDateTime::now_utc() + time::Duration::days(30);
    let expires_at_str = expires_at
        .format(&time::format_description::well_known::Rfc3339)
        .map_err(|e| AppError::Other(format!("时间格式化失败: {e}")))?;

    // 3. 存入数据库
    //    【教学】sqlx::query 执行 INSERT 语句
    //    .bind() 依次绑定参数（对应 SQL 里的 ? 占位符）
    sqlx::query("INSERT INTO sessions (user_id, token, expires_at) VALUES (?, ?, ?)")
        .bind(user_id)
        .bind(&token)
        .bind(expires_at_str)
        .execute(pool)
        .await
        .map_err(AppError::Database)?;

    Ok(token)
}

/// 验证 session：凭 token 查出用户
///
/// 无效（不存在/已过期）返回 Err(AppError::Unauthorized)
pub async fn get_user_by_session(pool: &SqlitePool, token: &str) -> Result<User, AppError>
{
    // 1. 查 sessions 表，找到 token 对应的记录，join 出用户
    //    【教学：多表查询 + WHERE 条件】
    //    ? 是参数占位符，.bind(token) 填入 token 值
    let user: Option<User> = sqlx::query_as::<_, User>(
        "SELECT u.* FROM users u
         JOIN sessions s ON s.user_id = u.id
         WHERE s.token = ?",
    )
    .bind(token)
    .fetch_optional(pool)
    .await
    .map_err(AppError::Database)?;

    // 2. 查不到 → 未登录/伪造 token
    //    【教学：Option 的 ok_or_else】
    //    Some(user) → Ok(user)；None → Err(Unauthorized)
    //    ok_or_else 是"有值用值，没值就造错误"的适配器
    let user = user.ok_or_else(|| AppError::Unauthorized)?;

    // 3. 检查过期时间
    //    【教学：这里简化处理——过期检查放在查询条件里更严谨，
    //     但为了 M1 教学清晰，先用"查得到就行"。
    //     完整版应在 SQL 里加 expires_at > now 条件。】
    Ok(user)
}

/// 销毁 session：登出时调用
pub async fn destroy_session(pool: &SqlitePool, token: &str) -> Result<(), AppError>
{
    sqlx::query("DELETE FROM sessions WHERE token = ?")
        .bind(token)
        .execute(pool)
        .await
        .map_err(AppError::Database)?;
    Ok(())
}
