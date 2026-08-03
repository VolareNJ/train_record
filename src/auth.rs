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
/// 【教学：为什么 generate 的参数是 &mut OsRng（可变引用）？】
/// 一句话：随机数生成器（RNG）是【有内部状态】的，
/// 生成一个随机数 = 推进（改变）这个状态，所以要可变访问权。
///
/// 类比抽奖机（老虎机）：机器内部有个转轮（状态）。
/// 每拉一次杆，转轮转一次，状态变了。
/// 想拉杆就必须"碰"这台机器；只允许"看"（&）的话，
/// 你只能看到转轮当前停在哪，拉不了杆。
///
/// 为什么"取下一个随机数"必然改变状态？
/// - 伪随机（如 ChaCha）：内部有种子/计数器，下一个数依赖上一个，
///   每次输出后计数器要更新，否则永远生成同一个数
/// - 真随机（从系统熵池读）：每次读取会消耗熵池，系统要跟踪剩余量
///
/// 这是 trait 签名决定的（rand_core::RngCore）：
///   fn next_u32(&mut self) -> u32
///   fn fill_bytes(&mut self, dest: &mut [u8])
/// SaltString::generate 内部要调 fill_bytes，所以参数必须是 &mut。
///
/// 编译期强制：写 SaltString::generate(&OsRng) 会报 E0596
/// （cannot borrow as mutable）——借用规则：要修改一个值必须持有 &mut。
///
/// 微妙点：OsRng 本身是单元结构体、没有内部状态（每次直接向 OS 请求），
/// 但 API 统一用 &mut，是为了兼容有状态的 RNG（如 StdRng）——
/// 同一个 trait 能接住任何 RNG 实现，调用方不用区分。
///
/// 为什么不用 RefCell 搞成 &self？可以，但把"运行时检查"搬进了每次调用，
/// 更慢还可能 panic；Rust 社区选择编译期就用 &mut 解决问题。
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
    // unimplemented!("M1 学生实现：密码哈希")

    let random_salt = SaltString::generate(&mut OsRng);
    let hashed = Argon2::default()
        .hash_password(plain.as_bytes(), &random_salt)
        .map_err(|e| AppError::Other(format!("密码哈希失败: {}", e)))?;
    Ok(hashed.to_string())
}

/// 校验密码：登录时用。返回 true = 密码正确
///
/// 【教学：为什么用 verify 而不是重新哈希比较？】
/// 因为哈希带随机盐，同一密码每次哈希结果都不同。
/// verify 是"用存下来的哈希串去验证输入的密码"，
/// 它会把盐从哈希串里提取出来，重新计算再比较。
/// 相当于"用原章验原件"，而不是"重新刻章对比"。
///
/// 【教学：带盐哈希到底怎么验证？（盐每次随机，怎么能比？）】
/// 核心一句话：验证时【不生成新盐】，而是把原来的盐从哈希串里
/// 提取出来复用。验证过程没有任何随机。
///
/// 先修正一个类比误区：盐不是"算法"，是"输入参数"。
/// - 算法是固定的：argon2 就是确定的函数 f(密码, 盐, 参数)
///   同一个函数，不是每次换一个（所以不是"每次用不同算法"）
/// - 确定性：同一函数 + 同一输入 → 永远同一输出
/// 验证时输入和注册时完全一致（同密码 + 同盐）→ 输出必然一致 → 可比。
///
/// 盐为什么能"复用"？因为它就藏在哈希串里（自包含）：
///   $argon2id$v=19$m=19456,t=2,p=1$NzZkNDM3...$YWViZGMxMjM0...
///   └─算法名┘ └版本┘└──参数──┘└─盐(base64)─┘└─哈希值(base64)─┘
/// 盐、算法版本、参数全在这一串里，随哈希一起存数据库，
/// 所以"找原章"不用额外存储，PasswordHash::new 解析就能拿到。
///
/// 流程：
///   注册：密码 + 随机盐 S1 ──► H1（S1 嵌在 H1 里）→ 数据库只存 H1
///   登录：输入密码 → 取 H1 → 解析提取盐 S1
///         → S1 + 输入密码 重新算 H'
///         → 比较 H' 与 H1 的哈希部分：相等 = 密码正确
/// 如果用户输错密码：S1 + 错密码 ──► 结果 ≠ H1 → 拒绝。
///
/// 那"盐每次随机"的意义在哪？服务于【存储时的不可预测性】：
/// - 没有盐：相同密码 → 相同哈希（一眼看出两人同密码）；
///   彩虹表（预计算密码→哈希的大表）直接命中，弱密码秒破
/// - 有随机盐：相同密码 → 不同哈希（看不出关联）；
///   彩虹表要为每颗盐各算一份，成本爆炸，只能逐个暴力破解
/// 一句话：盐防的是"攻击者偷懒"；验证靠的是"复用同一颗盐"的确定性，
/// 两者不冲突——随机只发生在生成那一刻，验证时是提取复用。
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
    // unimplemented!("M1 学生实现：密码校验")

    let parsed_hash =
        PasswordHash::new(hash).map_err(|e| AppError::Other(format!("解析哈希失败: {}", e)))?;
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
//
// 【教学：token 到底是什么？】
// 不是"签名加密后的 user_id"！token 只是一个【随机生成的编号】（uuid），
// 本身不携带任何用户信息。user_id 和它的对应关系存在数据库 sessions 表：
//   通行证编号(token)    持有者(user_id)    有效期(expires_at)
//   a3f2-9c1b-...（随机）  3                  2026-09-02
// 验证不是"解密出 user_id"，而是"查得到 = 有效，查不到 = 伪造/过期"。
//
// 关键：token 是【两方共有】的——
//   客户端 cookie 里有一份（复印件），服务器数据库里有一份（底账）。
// 销毁 session = 撕掉服务器这边的底账，客户端手里的复印件立刻变废纸
// （因为每次进门都要回来对账，对不上就是假的）。
//
// 【教学：为什么 destroy_session 传 token 而不是 user_id？】
// 因为一个用户可以有多个 session（手机、电脑、平板同时登录）：
//   sessions 表里 user_id=1 对应三行。
// 登出 = 只销毁【当前这台设备】的 session：
//   用 token 删：DELETE ... WHERE token = ?
//     → 精确删一行，其他设备不受影响 ✅
//   用 user_id 删：DELETE ... WHERE user_id = ?
//     → 该用户所有设备全部被登出 ❌
// 而且登出请求的 cookie 里本来就带着 token，服务器手头就有，直接用最自然。
//
// 【教学：查数据库方案 vs 签名验签方案】
// 有人会想：token 不应该是用 session_secret 把 user_id 加密签名出来的吗？
// 那是"签名验签方案"，本项目 M1 用的是"查数据库方案"，两者对比：
//   对比项        签名验签方案                查数据库方案（本项目）
//   token 生成   user_id 加密进 token        随机 uuid，与 user_id 无关
//   token 验证   secret 解签读出 user_id     拿 token 去 sessions 表查
//   session 数据 全在 token 里（服务器无状态） 存在数据库 sessions 表
//   登出          无法真正作废（只能靠黑名单）  DELETE 一行立即生效
//   session_secret 必须（签名的私章）          没用上（预留字段）
// 为什么 M1 选数据库方案？登出是真的登出 + 直观好懂；签名方案留到 M4+ 再讲。
// 一句话：本项目里 session_secret 目前是"装饰品"（预留字段），
//   token 是随机编号，user_id 和它的关系在数据库里，不在 token 里。
//
// 【教学：token 泄露了怎么办？】
// 答：任何拿到 token 的人都能冒充该用户——这是所有 token 方案的共性
// （签名方案也一样：签名只保证"通行证是服务器发的真货"，
//   不保证"持有人是本人"。门卫验的是章，不是人脸）。
// 所以安全设计的目标不是"泄露了也没事"（做不到），而是两条腿：
//   1. 让 token 很难被偷
//   2. 被偷了损失可控
// 防御手段：
//   随机不可预测（uuid）   → 防猜测/遍历（无法枚举别人的 token）
//   HTTPS 传输             → 防中间人窃听 cookie
//   HttpOnly cookie        → 防 XSS 脚本偷读
//   有效期 30 天           → 泄露的 token 会自己过期
//   服务器端可作废（DELETE）→ 发现泄露立即吊销
// 数据库方案在"泄露后补救"上反而赢签名方案：
//   签名方案的 token 发出去就收不回，只能等过期或维护黑名单；
//   数据库方案 DELETE 一行，下次请求就查不到，立即失效。
// 对比：token 泄露损失 < 密码泄露损失——
//   token 有时效、只对本站有效、可单独吊销；
//   密码永久有效、且常被跨站复用（泄露一个等于泄露全部）。
// 这就是为什么密码必须哈希存储（hash_password），而 token 可以存明文：
// 两者的威胁模型完全不同。
// ============================================================

/// 创建 session：登录成功后调用
///
/// 返回 token 字符串，调用方（handler）把它放进 cookie 发给浏览器
///
/// 【教学：sqlx 参数化查询（为什么用 ? + bind 而不是拼字符串？）】
/// 初学者容易把 sqlx::query 当 println! 用：
///   sqlx::query("INSERT ... VALUES ({}, {}, {})", a, b, c)  ❌
/// sqlx::query 只接受【一个】SQL 字符串参数，真正的流程是：
///   1. SQL 里用 ? 占位（不是 {}）
///   2. .bind(值) 按顺序把每个 ? 绑定上
///   3. .execute(pool).await 才真正执行（前面只是"搭好没跑"）
///
/// 为什么不能把值直接拼进 SQL 字符串？——【SQL 注入】
/// 如果值是用户输入，拼进去可能改变 SQL 结构：
///   用户输密码：' OR '1'='1
///   SELECT * FROM users WHERE password = '' OR '1'='1'   ← 恒真！绕过验证
/// bind 是参数化查询：值走独立通道传给数据库，
/// 永远不可能被当成 SQL 代码执行。这是数据库安全的底线。
///
/// 【教学：为什么 bind(&token) 而不是 bind(token)？（所有权移动）】
/// String 不实现 Copy（管理堆内存），.bind(token) 会把 token 的
/// 【所有权】移交进查询对象——就像把书借出去，自己手上就没书了。
/// 后面还要 Ok(token)，所以只能【借用】：
///   .bind(&token)   // 借出去用一下，用完后自己还留着
/// 记忆点：传给别人的值，如果后面还要用，就借（&）而不是交（move）。
///
/// 【教学：map_err 的两种等价写法】
///   .map_err(AppError::Database)          // 直接传构造函数（提示的风格）
///   .map_err(|e| AppError::Database(e))   // 闭包包装（另一种写法）
/// 两者 100% 等价：枚举变体构造器本身就是函数，
/// 类型是 fn(sqlx::Error) -> AppError，可以直接传给 map_err。
/// 区别只在场景：纯转换用前者（简洁）；需要附加逻辑
/// （拼错误信息、打日志）用后者。本项目两种写法都正确。
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
    // unimplemented!("M1 学生实现：创建 session")

    let new_token = uuid::Uuid::new_v4().to_string();
    let expire_dt = (time::OffsetDateTime::now_utc() + time::Duration::days(30))
        .format(&time::format_description::well_known::Rfc3339)
        .map_err(|e| AppError::Other(format!("计算时间失败: {}", e)))?;
    sqlx::query("INSERT INTO sessions (user_id, token, expires_at) VALUES (?, ?, ?)")
        .bind(user_id)
        .bind(&new_token)
        .bind(expire_dt)
        .execute(pool)
        .await
        .map_err(|e| AppError::Database(e))?;
    Ok(new_token)
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
