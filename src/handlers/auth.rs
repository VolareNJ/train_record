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
    extract::{Form, FromRequestParts, State},
    http::{
        HeaderMap,
        header::SET_COOKIE, // SET_COOKIE：设置响应头用（login/logout）
        request::Parts, // Parts：请求的"非 body 部分"（headers/method/uri...），提取器从这里拿数据
    },
    response::{Html, Redirect}, // 重定向：登录成功跳首页、登出跳登录页
};
use serde::Deserialize;

use crate::{
    AppState,
    auth::{self, get_user_by_session},
    error::AppError,
    models::User,
};
//   AppState   = 共享数据（连接池等），State(state) 提取器用它
//   auth       = 上一节实现的认证逻辑（hash/verify/create_session/...）
//   AppError   = 统一的错误类型，? 运算符自动转
//   User       = 用户表结构体，查询结果转成它

// ============================================================
// 【教学：为什么 handler 要 State<AppState>，而不是直接传连接池？】
// ============================================================
// 学生提问："login 需要查数据库，直接传 pool 不就行了？"
//
// 答案分三层：
//
// 1. handler 的签名不是我们定的，是 axum 的提取器系统定的。
//    handler 由 axum 调用，每个参数必须是"提取器"能识别的类型：
//    State<AppState>、Form<LoginForm>、HeaderMap ...
//    请求进来时 axum 按类型自动"提取"数据再调用 handler。
//    写 pool: SqlitePool 的话 axum 不知道它从哪来，编译直接报错。
//
// 2. AppState 是"工具箱"，pool 只是里面的扳手。
//    现在只有 pool + config 两个字段，以后还会加
//    （M2 的模板引擎、第三方客户端……）。
//    传 AppState 这个"工具箱"，加新依赖只改结构体字段，
//    所有 handler 签名一行不用动。传 pool 就要全改。
//
// 3. 对比普通函数：auth.rs 里的 get_user_by_session(pool, token)
//    直接收 pool——因为它是普通函数，我们自己调用，参数随便写。
//    handler 是 axum 调用的，只能走提取器通道。
//
// 在函数体里取用：let pool = &state.pool;  // 从工具箱里拿扳手
//
// 💡 思考题：require_user 是辅助函数，签名是 state: &AppState
//   而不是 State<AppState>，为什么？
//   （答：因为它不是 handler，是我们自己调用的普通函数，
//     不需要 axum 帮我们注入，直接传引用即可。）
// ============================================================

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
/// 【教学：为什么返回 Html 而不是 String —— 隐藏的 text/plain bug】
/// 2026-08-08 排查：登录页在浏览器里显示为纯文本（<pre> + 转义），
/// 但 curl 看到的是正常 HTML。原因：axum 对 String 的默认 Content-Type
/// 是 text/plain（纯文本）！浏览器收到纯文本就把 <form> 当普通文字显示。
/// 必须用 Html 包裹，axum 才会返回 text/html（浏览器才渲染）。
///
/// 教训：本地开发用 curl 测试页面，curl 不渲染只显示 body，
/// 看不到 text/plain 的问题；只有在浏览器里打开才发现。
/// 这也是为什么教学注释说 M2 换 askama 模板（自动处理类型）。
pub async fn login_page() -> Html<String>
{
    Html(
        r#"
        <head><meta name="viewport" content="width=device-width, initial-scale=1.0"></head>
        <form method="post" action="/login">
          <label>用户名 <input name="username" required></label><br>
          <label>密码 <input name="password" type="password" required></label><br>
          <button type="submit">登录</button>
        </form>
        "#
        .to_string(),
    )
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
/// 【教学：fetch_optional 的"套娃"类型 —— Result<Option<User>, sqlx::Error>】
/// 学生提问："fetch_optional 返回的 Option，map_err 怎么作用在它前面？"
///
/// 关键：fetch_optional 返回的不是 Option，而是两层套在一起：
///
///   Result<Option<User>, sqlx::Error>
///   ↑外层：数据库查询"成功了吗"？（系统层面）
///     ↑内层：用户"存在吗"？（业务层面）
///
/// 拆两层，各用一个 ?：
///   1. 外层 Result → map_err 换错误类型 + ? 拆开
///      （sqlx::Error → AppError::Database，表示"数据库坏了"）
///   2. 内层 Option → ok_or_else + ? 拆开
///      （None → AppError::Validation，表示"查无此人"）
///
/// 为什么"查不到用户"也要走 Err 通道？
///   Err 通道既承载"系统故障"，也承载"业务拒绝"：
///   - 数据库坏了      → AppError::Database   （浏览器看到 500 错误）
///   - 查无此人/密码错  → AppError::Validation  （浏览器看到登录失败）
///   查不到用户不是程序出错，而是登录流程的业务失败，
///   但 handler 只有 Ok/Err 两条路，所以只能走 Err 通道把"登录失败"送回浏览器。
///
/// ok_or vs ok_or_else（学生踩过的坑）：
///   ok_or(值)        = 直接传错误值（提前构造好了）
///   ok_or_else(|| 值) = 传零参数闭包，只有 None 时才执行（惰性构造）
///   注意闭包参数数量：map_err 的闭包要 1 个参数 |e|，
///   ok_or_else 的闭包要 0 个参数 ||，别搞混。
///
/// 完整链条：
///   fetch_optional().await      → Result<Option<User>, sqlx::Error>
///     .map_err(AppError::Database)?  → Option<User>（数据库故障提前返回）
///     .ok_or_else(|| Validation)?    → User（查无此人提前返回）
///
/// 【教学：verify_password 的 Ok(false) 是答案，不是错误】
/// 学生踩坑："密码不对"想用 map_err 转成 Validation，方向错了。
/// verify_password 返回 Result<bool, AppError>：
///   Ok(true)  = 密码正确（正常结果）
///   Ok(false) = 密码错误（也是正常结果！走 Ok 通道）
///   Err(...)  = 哈希解析失败（系统内部错误，如数据库存了损坏的哈希）
///
/// 正确写法（本项目）：
///   let is_correct_password = auth::verify_password(&form.password, &user.password_hash)?;
///   if !is_correct_password { return Err(Validation("用户名或密码错误")); }
///
/// 为什么不能 map_err？
///   map_err 只处理 Err 变体，但密码错误时返回的是 Ok(false)，根本不会经过它。
///   而且 Err 里装的是"哈希解析失败"（系统问题），
///   把它伪装成"用户名或密码错误"（业务问题）会误导排查。
///
/// 💡 经典模式：Result<bool, _> 里，bool 承载业务判断，Err 承载系统故障。
///    返回"是/否"的函数，判断逻辑写在 Ok 分支外，别把 false 塞进 Err。
///
/// 【教学：&str vs &[u8] —— 为什么不能传 as_bytes()】
/// 学生提问："verify_password 要 &str，str 底层不就是字节吗，as_bytes() 不行吗？"
///
/// 答案：底层都是字节，但类型系统严格区分：
///   &str  = 带"UTF-8 合法"保证的字节序列（胖指针：指针+长度）
///   &[u8] = 任意字节序列（可能不是合法 UTF-8）
///
/// 类比：&str 是贴着"UTF-8 质检标签"的包裹，&[u8] 是裸包裹。
/// 函数收货单写"只收带标签的"，递裸包裹过去编译器拒收：
///   expected `&str`, found `&[u8]`（编译报错）
///
/// 转换方向是单向的：
///   &str → &[u8]  永远安全（.as_bytes() 无损，str 本来就是字节）
///   &[u8] → &str  可能失败（from_utf8() 返回 Result，字节可能是乱码）
///
/// 所以 verify_password 入口收更严格的 &str，内部再 .as_bytes() 转给 argon2。
/// 入口严格，调用方就不可能传进非法 UTF-8。
///
/// 【教学：deref coercion —— &String 为什么能当 &str 用】
/// 传 &form.password（&String）时编译器自动转换：
/// String 实现了 Deref<Target = str>，&String 在需要 &str 处自动 deref。
/// 这是隐式转换，不是手动转。
///
/// 签名写 &str 而不是 &String 的好处（Rust 惯例）：
///   传 &String   → 自动 deref ✅
///   传字符串字面量 "abc"（就是 &str）→ 直接用 ✅
///   传 &str 变量  → 直接用 ✅
/// 写 &String 的话，后两种全都不行。
///
/// 【教学：sqlx 链式调用每一步的类型（rust-analyzer 不可用时看这里）】
/// 口诀：.bind() 类型不变 → .fetch_*()/.execute() 返回 Future
///       → .await 变 Result → ? 拆成值（并传播错误）
///
/// 查询单行（query_as + fetch_optional，本函数用）：
///   sqlx::query_as::<_, User>("SELECT ... WHERE username = ?")
///     → QueryAs<'_, Sqlite, User, _>          ① 查询"构建器"，还没执行
///   .bind(&form.username)
///     → QueryAs<'_, Sqlite, User, _>          ② 类型不变！bind 只是塞参数
///   .fetch_optional(&state.pool)
///     → Future<Result<Option<User>, sqlx::Error>>  ③ 进入异步
///   .await
///     → Result<Option<User>, sqlx::Error>     ④ await 解包 Future → Result
///   .map_err(AppError::Database)?
///     → Option<User>                          ⑤ 换错误类型 + ? 拆开
///
/// 查询多行（query_as + fetch_all，admin_users 用）：
///   sqlx::query_as::<_, User>("SELECT ... ORDER BY id")
///     .fetch_all(&state.pool).await
///     → Result<Vec<User>, sqlx::Error>        （Ok 里是 Vec<User>）
///
/// 执行增删改（query + execute，admin_create_user 用）：
///   sqlx::query("INSERT INTO users ... VALUES (?, ?, ?)")
///     → Query<'_, Sqlite, _>                  ① 构建器
///   .bind(...).bind(...).bind(...)
///     → Query<'_, Sqlite, _>                  ② 类型不变
///   .execute(&state.pool).await
///     → Result<SqliteQueryResult, sqlx::Error>  ③ SqliteQueryResult = 受影响行数
///
/// 核心区别：
///   query_as  = 按 FromRow 把每行转成结构体（fetch_optional → Option<T>，
///               fetch_all → Vec<T>，fetch_one → 恰好一行否则报错）
///   query     = 不转换，只执行（增删改）
///   query_as::<_, User> 里第一个 _ 让编译器推断参数类型，第二个 User 是目标类型。
///
/// 【教学：map_err 要 1 参数，ok_or_else 要 0 参数 —— 为什么？】
/// 不是约定俗成，而是由"变体带不带值"决定的：
///   enum Result<T, E> { Ok(T), Err(E) }   // Err 里装着 E → map_err 闭包收 e
///   enum Option<T>    { Some(T), None }   // None 里啥都没有 → ok_or_else 不收参数
///
/// 所以看闭包参数数量，其实是在问"这个变体有没有值要传给你"：
///   map_err  |e|  处理 Err(e) 里的错误值
///   ok_or_else ||  None 变体没有值，闭包只需"生成"一个错误返回
///
/// 学生踩过的坑：
///   1. map_err 写成 ||  →  E0593：closure expected 1 argument
///   2. ok_or_else 写成 |e| → 同样 E0593（反过来）
///   3. AppError::Other("...") 传 &str → 要 String，得 .to_string()
///
/// 【教学：链式调用"返回值劫持类型"陷阱 —— HeaderMap::new().insert()】
/// 学生写成：let mut headers = HeaderMap::new().insert(...);
/// 结果 headers 的类型不是 HeaderMap，而是 Option<HeaderValue>！
/// 原因：insert 的返回值是"被覆盖的旧值"（Option<HeaderValue>），
/// 链式创建时，变量的类型被"最后一个方法的返回值"劫持了。
///
/// 正确写法：先创建，再操作，两步分开：
///   let mut headers = HeaderMap::new();        // headers: HeaderMap
///   headers.insert(...);                        // 返回值丢弃（我们不 care）
///
/// 同类坑：Vec::new().push(x) —— push 返回 ()，变量类型变成 ()。
/// 💡 记住：创建和操作分开写，除非你确实想要返回值。
///
/// 【教学：Ok 里装元组要整体加括号】
/// 返回类型 Result<(HeaderMap, Redirect), AppError> 的 Ok 里是一个元组：
///   Ok((headers, Redirect::to("/")))   ✅
///   Ok(headers, Redirect::to("/"))     ❌ 这是把 Ok 当多参数函数用了
/// （多值返回 = 元组，元组 = 一个值，括号必须包住整体）
///
/// 【教学：cookie.parse() 是干什么的？】
/// 学生提问："组装好的 cookie 是 String，为什么要 parse？"
///
/// 核心：你组装的是 String（字符串），但 headers.insert 要的是
/// HeaderValue（HTTP 头专用类型）。parse 就是"字符串 → HeaderValue"的桥梁：
///   let cookie = format!("session={token}; ...");  // cookie: String
///   cookie.parse()                                  // → Result<HeaderValue, _>
///
/// 为什么不能直接 insert String？
/// HeaderValue 要保证值是合法 ASCII/可见字符、不能有换行
/// （防 HTTP 响应头注入攻击），所以必须经过 parse 校验：
///   合法 → Ok(HeaderValue)
///   非法（如含换行符）→ Err
///
/// 为什么我们的 token 必然合法？uuid 只含十六进制字符和连字符，
/// 必过校验。但防御性编程——即使知道数据安全，也要处理 Err 分支，
/// 将来可能有人改代码传别的东西进来。
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
    // unimplemented!("M1 学生实现：登录")

    let user_op = sqlx::query_as::<_, User>("SELECT * FROM users WHERE username = ?")
        .bind(&form.username)
        .fetch_optional(&state.pool)
        .await
        .map_err(|e| AppError::Database(e))?;

    let user = user_op.ok_or_else(|| AppError::Validation("用户名不存在或密码错误".to_string()))?;

    let is_correct_password = auth::verify_password(&form.password, &user.password_hash)?;

    if !is_correct_password
    {
        return Err(AppError::Validation("用户名不存在或密码错误".to_string()));
    }

    // 4. 创建 session：let token = auth::create_session(&state.pool, user.id).await?;
    let token = auth::create_session(&state.pool, user.id).await?;

    // 5. 组装 cookie：
    //    format!("session={token}; Path=/; HttpOnly; SameSite=Lax; Max-Age=2592000")
    //    （Max-Age=2592000 秒 = 30 天，与 session 过期一致）
    let cookie = format!(
        "session={}; Path=/; HttpOnly; SameSite=Lax; Max-Age=2592000",
        token
    );

    // 6. 放入响应头：
    //    let mut headers = HeaderMap::new();
    //    headers.insert(SET_COOKIE, cookie.parse().map_err(...)?);
    let mut headers = HeaderMap::new();
    headers.insert(
        SET_COOKIE,
        cookie
            .parse()
            .map_err(|_e| AppError::Other("头文件生成失败".to_string()))?,
    );

    // 7. 返回 (headers, Redirect::to("/"))
    // 返回类型说明：登录成功返回「响应头 + 重定向」的元组
    // （响应头里放 Set-Cookie，重定向到首页）
    Ok((headers, Redirect::to("/")))
}

// ============================================================
// 登出（POST /logout）
// ============================================================
/// 处理登出：销毁 session，清除 cookie，回到登录页
///
/// 【教学：为什么 logout 里不要自己 headers.get，而要调 extract_token？】
/// 学生提问："我 headers.get(COOKIE) 已经拿到 cookie 了，
///   为什么 extract_token 还要传整个 headers？"
///
/// 因为你拿到的只是"整个 Cookie 头的原始字符串"：
///   Cookie: foo=bar; session=abc123def; theme=dark
/// 一个 Cookie 头可以塞很多个 cookie（分号分隔），session 只是其中一个。
/// 需要 extract_token 做"从整串里找到 session 项并抠出 token"的解析。
///
/// 类型关键：
///   headers.get(COOKIE) 返回 Option<&HeaderValue>（还没解析）
///   extract_token(&headers) 自己会再调一次 headers.get(COOKIE) + 完整解析
///
/// 正确用法：直接调 extract_token(&headers) 一步到位，
/// 不要自己手动 get（那是在绕过工具、重复劳动）。
/// 这正是"把取 token 封装成函数"的意义：logout 和 require_user
/// 共用同一个解析逻辑（单一职责），而不是各自手写一遍。
///
/// 【教学：ok_or_else 强制 vs if let 跳过 —— 判断标准】
/// 学生初版写了 ok_or_else(|| AppError::Other("No token"))?，
/// 要求必须带 token 否则报错。但对 logout 来说这不对。
///
/// 判断标准：这个条件缺失，是"拒绝请求"还是"只是没活可干"？
///   login：必须查得到用户、必须密码对
///     → 缺失 = 拒绝请求 → ok_or_else(...)? 强制，失败就 Err
///   logout：没有 token 也能正常登出（比如 cookie 已过期）
///     → 缺失 = 没活可干 → if let Some 温柔跳过，统一走清除 cookie
///
/// 对比：
///   login 的逻辑："这请求不合格，我要拒绝它" → ? 强制
///   logout 的逻辑："没 token 就算了，照样把用户送回登录页" → if let 跳过
///
/// 💡 判断口诀：问自己"没这个东西，用户算不算'非法访问'"。
///    算 → 用 ? 强制；不算 → 用 if let 跳过。
///
/// 【教学：为什么新建 HeaderMap，而不是复用请求头？】
/// 学生尝试在请求头 headers（&HeaderMap，不可变）上直接 insert，
/// 编译报错。即使改成 mut 也不行，因为设计上就不该复用：
///
/// 请求头包含 Cookie、User-Agent、Accept 等一大堆
/// "浏览器发给服务器"的信息。这些不应该原样返回给浏览器。
/// 响应头应该只包含"服务器想告诉浏览器"的信息（如 Set-Cookie）。
///
/// 所以登录/登出都新建一个干净的 HeaderMap，只放 Set-Cookie。
/// 这也是为什么 handler 参数里 headers: HeaderMap（不可变）——
/// 它是"读"请求头用的，不是"写"响应头用的。
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
    // unimplemented!("M1 学生实现：登出")

    // 1. 从请求头里取 Cookie 头：headers.get(axum::http::header::COOKIE)
    // 2. 解析出 session=xxx 的 token（可复用下面的 extract_token 辅助函数）
    // 3. 有 token → auth::destroy_session(&state.pool, token).await?
    if let Some(token) = extract_token(&headers)
    {
        auth::destroy_session(&state.pool, &token).await?;
    }
    // 4. 设置清除 cookie：let cookie = "session=; Path=/; HttpOnly; SameSite=Lax; Max-Age=0";
    //    （Max-Age=0 = 立即过期，浏览器删除这个 cookie）
    let new_cookie = "session=; Path=/; HttpOnly; SameSite=Lax; Max-Age=0".to_string();
    // 5. 返回 (headers, Redirect::to("/login"))
    // 返回类型说明：登出返回「响应头 + 重定向」的元组
    // （响应头里放清除 cookie，重定向到登录页）
    let mut new_header = axum::http::HeaderMap::new();
    new_header.insert(
        SET_COOKIE,
        new_cookie
            .parse()
            .map_err(|_| AppError::Other("头文件生成失败".to_string()))?,
    );
    Ok((new_header, Redirect::to("/login")))
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
/// 【教学：extract_token 为什么收 &HeaderMap 而不是收 cookie 字符串？】
/// 因为解析逻辑只依赖"原始 Cookie 头"，而 cookie 头的唯一来源就是 headers。
/// 收 &HeaderMap 让调用方不用先自己 get 再传——
/// 这个函数自己完成"取头 → 解析 → 抠 token"的全过程，封装更彻底。
/// 调用方（logout/require_user）只需一句 extract_token(&headers)。
///
/// 【教学：and_then 是 Option 的方法，闭包处理 Some 里的值】
/// 学生确认："and_then 里的闭包是作用于 Some 变体里面的参数是吧？"
/// 对，完全正确：
///   Some(t).and_then(|x| f(x))   // → f(t)，闭包收到的是"解开后的值 t"（类型 T）
///   None.and_then(|x| f(x))      // → None，短路，闭包不执行
/// 闭包接收的是值本身，不是 Option 本身。
///
/// 【教学：闭包参数数量 = "变体里装的值"（统一规律）】
///   map        Option<T>    1 参数：Some 里的值    → 闭包返回普通值 U
///   and_then   Option<T>    1 参数：Some 里的值    → 闭包返回 Option<U>
///   map_err    Result<T,E>  1 参数：Err 里的值     → 闭包返回错误类型
///   ok_or_else Option<T>    0 参数                  → None 里没值，闭包凭空造错误
/// 心智模型：闭包要几个参数，就看这个变体里装了几个值。
/// Some(t) 装一个 → 1 参数；None 装零个 → 0 参数。
///
/// 【教学：map vs and_then —— 区别只在闭包的返回值】
///   Some(5).map(|x| x + 1)          // → Some(6)     闭包返回普通值
///   Some(5).and_then(|x| Some(x + 1)) // → Some(6)    闭包返回 Option，被压平
///
/// 为什么 extract_token 用 and_then 而不是 map？
///   .and_then(|v| v.to_str().ok())   // to_str().ok() 返回的就是 Option<&str>
///   // 用 map 会得到 Option<Option<&str>>（两层套娃），and_then 压平成一层
/// 💡 判断口诀：闭包返回普通值 → map；闭包返回 Option → and_then（否则套娃）。
///    and_then 的别名就叫 flatMap（展平映射），功能相同。
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
    // unimplemented!("M1 学生实现：解析 cookie token")
    // ============================================================
    // 【headers 样例】假设浏览器发来的请求头是：
    //   Cookie: foo=bar; session=550e8400-e29b-41d4-a716-446655440000; theme=dark
    //   User-Agent: Mozilla/5.0 ...（其他头不关心，我们只取 Cookie 这个头）
    //
    // 链上每一步的中间结果（对照下面实现看）：
    //   ① headers.get(COOKIE)      → Some("foo=bar; session=550e...; theme=dark")  （&HeaderValue）
    //   ② v.to_str().ok()          → Some("foo=bar; session=550e...; theme=dark")  （&str）
    //   ③ s.split(';')             → ["foo=bar", " session=550e...", " theme=dark"]
    //   ④ .map(str::trim)          → ["foo=bar", "session=550e...", "theme=dark"]
    //   ⑤ .find(以 "session=" 开头) → Some("session=550e8400-e29b-41d4-a716-446655440000")
    //   ⑥ .split('=').nth(1)       → Some("550e8400-e29b-41d4-a716-446655440000")
    //   ⑦ .map(to_string)          → Some("550e8400-e29b-41d4-a716-446655440000")  （String，函数最终返回值）
    //
    // 如果浏览器没带 Cookie 头：① 直接返回 None，整条链短路，函数返回 None。
    // ============================================================
    headers
        .get(axum::http::header::COOKIE) // ① Option<&HeaderValue>
        .and_then(|v| v.to_str().ok()) // ② Option<&str>
        .and_then(|s| {
            s.split(';') // ③ Iterator<Item=&str>
                .map(str::trim) // ④ Iterator<Item=&str>
                .find(|part| part.starts_with("session=")) // ⑤ Option<&str>
                .and_then(|part| part.split('=').nth(1)) // ⑥ Option<&str>
                .map(|t| t.to_string()) // ⑦ Option<String>
        })
}

/// 从请求头验证登录，返回当前用户（未登录 → Unauthorized）
///
/// 【教学：为什么最后一行不用 ?（尾表达式 vs ? 解包）】
/// 学生提问："get_user_by_session(...).await 返回 Result，
///   前面 extract_token 都用了 ?，为什么这里不用？"
///
/// 三个层面：
///
/// 1. 尾表达式（trailing expression）：
///    函数体"最后一行、没有分号"的表达式叫尾表达式，
///    它的值会被隐式返回，等价于 return 它的值。
///    所以 require_user 的返回值 = get_user_by_session(...).await 的结果，
///    不需要写 return，也不该加分号（加了分号就变成语句，值被丢弃）。
///
/// 2. 类型本来就匹配，? 无事可做：
///    ? 的两步工作——"解包 Result" + "错误类型转换（From）"。
///    这里 get_user_by_session 返回 Result<User, AppError>，
///    require_user 签名也是 Result<User, AppError>：
///      错误类型相同 → 不需要 From 转换
///      想要的就是整个 Result → 不需要解包
///    ? 没有工作要做，所以直接交付整个 Result。
///
/// 3. 如果用 ? 会怎样？（对比）
///    let user = auth::get_user_by_session(&state.pool, &token).await?;
///    Ok(user)
///    ? 把 Result 解包成 User，最后还得 Ok(user) 再包回去——绕了一圈。
///
/// 💡 判断口诀：看"错误类型需不需要转换"。
///    函数体中间的行：错误要提前返回 → 用 ?（转换 + 提前 return）。
///    尾表达式：直接交付 → 不用 ?（返回值就是整个 Result）。
///
/// 【实现步骤】
/// 1. let token = extract_token(headers).ok_or(AppError::Unauthorized)?;
/// 2. auth::get_user_by_session(&state.pool, &token).await
///    （注意传 &token：get_user_by_session 收 &str，token 是 String，
///      必须传引用才能匹配参数类型）
pub async fn require_user(state: &AppState, headers: &HeaderMap) -> Result<User, AppError>
{
    // TODO(M1): 学生实现（步骤见上方注释）
    // unimplemented!("M1 学生实现：路由守卫")
    let token = extract_token(headers).ok_or(AppError::Unauthorized)?;
    auth::get_user_by_session(&state.pool, &token).await
}

// ============================================================
// 用户管理页（GET /admin/users，仅管理员）
// ============================================================
/// 显示用户列表 + 创建用户表单（仅管理员可访问）
///
/// 【教学：整个函数是"三步走"】
///   ① 守卫：没登录/不是管理员 → 直接 Err
///   ② 查数据：fetch_all → Vec<User>
///   ③ 拼页面：把 Vec<User> 变成 HTML 字符串返回
/// 前两步是"验证 + 拿数据"，第三步是"渲染"，
/// 以后的 handler 基本都是这个套路：守卫 → 拿数据 → 渲染。
///
/// 【教学：迭代器 + 适配器（函数式拼 HTML）—— 每一步的类型】
///   users.iter()                    // Vec<User> → Iter<'_, User>（借用遍历，不拿所有权）
///     .map(|u| format!("<tr>..."))   // 每个用户 → 一行 HTML（User → String）
///     .collect::<Vec<_>>()           // 迭代器 → Vec<String>（map 惰性，不 collect 不干活）
///     .join("\n")                    // Vec<String> → 一个大 String（换行连接）
///
/// 为什么 map 是"惰性"的？
///   map 只是"登记了转换规则"，还没真正执行；
///   只有 collect/sum/for 这类"消费迭代器"的操作才会驱动它跑起来。
///   所以链式写法里 collect 必不可少。
///
/// 最终 user_vec 长这样（一行一个用户）：
///   "<tr><td>1</td><td>admin</td><td>管理员</td></tr>\n<tr><td>2</td><td>bob</td><td>普通用户</td></tr>"
///
/// 【教学：r#"..."# raw string（原始字符串）—— 为什么拼 HTML 用它】
/// HTML 里全是 < > " 这些字符，普通字符串每个引号都要转义：
///   "<form method=\"post\" action=\"/admin/users\">"   ← 噩梦
/// raw string 中间所有字符原样保留，零转义：
///   r#"<form method="post" action="/admin/users">"#   ← 干净
/// 规则：r#" 开头、"# 结尾，中间内容原样输出。
///
/// 【教学：format! 占位符 —— {} 与 {变量名} 两种写法等价】
///   format!("...{}...", user_vec)   // 老写法：按顺序对应参数
///   format!("...{user_vec}...")     // 新写法：直接捕获同名变量（Rust 1.58+）
/// 本项目用 {} + 参数列表，参考实现用 {rows} 捕获，效果一样。
///
/// 💡 坑提醒：如果 HTML 里有 CSS 花括号（如 <style>.x{color:red}</style>），
///    format! 会把 {color:red} 误认成占位符。需写成 {{ 和 }} 转义。
///    本项目 HTML 没有花括号，安全。
///
/// 【教学：map_err 传函数 vs 传闭包 —— 等价写法】
///   .map_err(AppError::Database)        // 元组构造器直接当函数用
///   .map_err(|e| AppError::Database(e)) // 等价的闭包写法
/// AppError::Database 是"元组结构体变体"，本身就是一个函数
/// （fn(sqlx::Error) -> AppError），所以能直接传给 map_err。
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
) -> Result<Html<String>, AppError>
{
    // TODO(M1): 学生实现（步骤见上方注释）
    let user = require_user(&state, &headers).await?;

    if !user.is_admin
    {
        return Err(AppError::Forbidden("需要管理员权限".to_string()));
    }

    let users = sqlx::query_as::<_, User>("SELECT * FROM users ORDER BY id ASC")
        .fetch_all(&state.pool)
        .await
        .map_err(AppError::Database)?;

    let user_vec = users
        .iter()
        .map(|u| {
            format!(
                "<tr><td>{}</td><td>{}</td><td>{}</td></tr>",
                u.id,
                u.username,
                if u.is_admin
                {
                    "管理员"
                }
                else
                {
                    "普通用户"
                }
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    Ok(Html(format!(
        r#"<!DOCTYPE html>
                <html lang="zh">
                <head><meta charset="UTF-8"><meta name="viewport" content="width=device-width, initial-scale=1.0"><title>用户管理</title></head>
                <body>
                <h1>用户管理</h1>
                <table border="1">
                    <tr><th>ID</th><th>用户名</th><th>角色</th></tr>
                    {}
                </table>
                <h2>创建新用户（邀请制）</h2>
                <form method="post" action="/admin/users">
                    <label>用户名 <input name="username" required></label><br>
                    <label>密码 <input name="password" type="password" required></label><br>
                    <label><input type="checkbox" name="is_admin" value="1"> 设为管理员</label><br>
                    <button type="submit">创建用户</button>
                </form>
                <p><a href="/">返回首页</a></p>
            </body>
            </html>"#,
        user_vec
    )))
}

// ============================================================
// 创建用户（POST /admin/users，仅管理员）
// ============================================================
/// 管理员创建新用户（邀请制：只有管理员能创建账号）
///
/// 【教学：is_admin 的两种等价写法 —— match vs as_deref】
/// 学生提问："as_deref 不太理解，改成了 match 逻辑"
///
/// 背景：form.is_admin 的类型是 Option<String>
/// （checkbox 勾选时表单提交 is_admin=1，没勾选就没有这个字段）。
///
/// 写法 A（match，学生版）：
///   let is_admin = match form.is_admin
///   {
///       Some(v) => v == "1",   // v: String，String 实现了 PartialEq<&str>，可直接和 "1" 比
///       None => false,
///   };
///
/// 写法 B（as_deref 一行，参考版）：
///   let is_admin = form.is_admin.as_deref() == Some("1");
///
/// 两者行为完全一致，都是"值是 "1" → true，否则 false"。
///
/// as_deref 是什么？= "Option<T> 的 as_str"：
///   String::as_str()        → String → &str（取出借用）
///   Option<String>.as_deref() → Option<String> → Option<&str>（把里面的值也变借用）
/// 规则：只要 T 实现了 Deref，Option<T>.as_deref() 就是 Option<&T::Target>。
///   String: Deref<Target = str> → Option<String>.as_deref() = Option<&str>
/// 两边类型一致（Option<&str> == Option<&str>），才能直接 ==。
///
/// 💡 学生踩过的坑：写成 Some(1) => true 会报 E0308——
///    form.is_admin 装的是 String 不是整数，Some 里必须匹配字符串。
///    match 匹配的是"盒子里装的类型"，不是"你想要的语义"。
///
/// 【教学：校验边界 —— len() < 6 vs len() <= 6】
///   < 6   = 6 位及以上放行（"至少 6 位"）
///   <= 6  = 7 位及以上才放行（要求 7 位，比约定更严）
/// 本项目约定"密码至少 6 位"，所以用 < 6。
///
/// 【教学：trim().is_empty() —— 全空格用户名也是空】
///   form.username.is_empty()       → 只查"长度是否为 0"
///   form.username.trim().is_empty() → 先去空格再查，"   " 这种也算空
/// trim() = 去掉字符串首尾的空白字符（空格、换行、Tab）。
///
/// 【教学：INSERT 语句 + 参数绑定】
/// sqlx::query("INSERT INTO users (username, password_hash, is_admin) VALUES (?, ?, ?)")
///     .bind(&form.username).bind(&password_hash).bind(is_admin)
///     .execute(&state.pool).await
/// ? 占位符依次对应 bind 的参数：username → password_hash → is_admin。
/// bind 不检查 SQL 类型，只负责"按顺序塞参数"（防 SQL 注入靠占位符机制本身）。
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
///
/// 【教学：401 vs 403 —— 两个"拒绝"的区别】
///   require_user 失败          → 401（还没登录，你是谁？）
///   is_admin 为 false          → 403（登录了但权限不够，你不配）
///   两者都是"拒绝"，但语义不同，HTTP 状态码也不同。
///   旧代码用 Validation（422 参数不合法）是偷懒——权限问题和
///   表单校验混为一谈；正确语义是 Forbidden（403）。
pub async fn admin_create_user(
    State(state): State<AppState>,
    headers: HeaderMap,
    Form(form): Form<CreateUserForm>,
) -> Result<Redirect, AppError>
{
    // TODO(M1): 学生实现（步骤见上方注释）
    let user = require_user(&state, &headers).await?;

    if !user.is_admin
    {
        return Err(AppError::Forbidden("需要管理员权限".to_string()));
    }

    if form.username.trim().is_empty() || form.password.len() < 6
    {
        return Err(AppError::Other("用户名为空或密码小于6位".to_string()));
    }

    let password_hash = auth::hash_password(&form.password)?;

    let is_admin = match form.is_admin
    {
        Some(v) => v == "1",
        None => false,
    };

    sqlx::query("INSERT INTO users (username, password_hash, is_admin) VALUES (?, ?, ?)")
        .bind(form.username)
        .bind(password_hash)
        .bind(is_admin)
        .execute(&state.pool)
        .await
        .map_err(AppError::Database)?;

    Ok(Redirect::to("/admin/users"))

    // unimplemented!("M1 学生实现：创建用户")
}

// ============================================================
// 【M2 第 1 步】AuthUser 提取器（方案 A：声明式守卫）★ 本阶段核心
// ============================================================
// 【教学：为什么 M2 要引入 AuthUser？】
// M1 的守卫写法（方案 B）：
//   async fn list(State(state): State<AppState>, headers: HeaderMap)
//       -> Result<..., AppError>
//   {
//       let user = require_user(&state, &headers).await?;   // ← 每个函数都写
//       ...
//   }
// M2 有 14 个 handler 都要登录，照 M1 写法每个函数都要重复两行：
//   headers: HeaderMap 参数 + require_user(&state, &headers).await?
// 重复代码一多，就值得"封装"了。
//
// 【教学：声明式 vs 命令式 —— 守卫的两种哲学】
//   方案 B（命令式）：
//     函数体里自己调用 require_user，"拿到再继续"——
//     像保安在门口手动盘查，每个房间门口都要站一个保安。
//
//   方案 A（声明式）：
//     签名里写 user: AuthUser，axum 在调用 handler 之前自动做守卫——
//     像大厦门禁系统：你刷了卡才能进，进门的事由系统管，
//     办公室里不用再设保安。
//
//   axum 里"声明式"的实现方式，就是实现 FromRequestParts：
//   告诉 axum"AuthUser 这种参数，从请求里怎么提取出来"。
//   handler 签名里出现 AuthUser，axum 就会自动调用我们的提取逻辑。
//   M1 我们剖析过 State/HeaderMap 的提取器机制，现在自己实现一个。
//
// 【教学：FromRequestParts vs FromRequest —— 为什么守卫用 parts 版】
//   FromRequestParts：只能读请求的"非 body 部分"（headers、method、uri...）
//   FromRequest：    能读整个请求（包括 body）
//   守卫只需要读 Cookie 头，不碰 body，所以用 FromRequestParts 就够了。
//   好处：parts 版可以和其他需要 body 的提取器（如 Form）同时用，
//         请求体不会被抢走（body 只能被一个提取器消费）。
//
// 【教学：Rejection = AppError —— 提取器失败返回什么】
//   type Rejection = AppError;
//   意思是：提取失败时返回 AppError（这里就是 Unauthorized 401）。
//   为什么这样设计？handler 的返回类型是 Result<_, AppError>，
//   提取器返回的 Rejection 必须能转成 handler 的错误类型，? 才能用。
//   我们直接让 Rejection = AppError，两边一致，零转换。
//   （这也意味着：AuthUser 提取失败 → 401 → 前端自动跳登录页。）
//
// 【教学：元组结构体 AuthUser(pub User) —— 包装器模式】
//   为什么不直接返回 User？因为提取器是按"参数类型"匹配的，
//   如果直接写 user: User，axum 无法区分"这是提取器"还是普通类型。
//   包一层 AuthUser 就是告诉 axum："这个类型是我的自定义提取器"。
//   用的时候模式解构：
//     async fn list(State(state): State<AppState>, AuthUser(user): AuthUser)
//   解构后 user 就是 User，和 M1 的 require_user 返回的一样。
//
// 【教学：复用 vs 重写 —— 提取器只是"换个入口"】
//   AuthUser 提取器内部做的事，和 require_user 一模一样：
//     extract_token(&parts.headers) → get_user_by_session(&state.pool, token)
//   我们直接复用这两个既有函数，不重新写解析逻辑。
//   这就是分层的好处：逻辑层（auth.rs）写好一次，
//   表现层（守卫函数、提取器）想用几次用几次。
//   require_user 本身保留：admin 页面还要用它（它返回 User 方便 .is_admin），
//   以及作为教学对照。
//
// 【实现步骤】（学生填写 impl 块里的函数体）
// 1. 取 token：
//      let token = extract_token(&parts.headers).ok_or(AppError::Unauthorized)?;
//    （extract_token 收 &HeaderMap，parts.headers 就是 HeaderMap）
// 2. 查用户：
//      let user = auth::get_user_by_session(&state.pool, &token).await?;
// 3. 包一层返回：
//      Ok(AuthUser(user))
//
// 📌 M1 学习目标验收：能说出"Rejection 为什么用 AppError"、
//   "为什么是 FromRequestParts 而不是 FromRequest"。
//
// ============================================================
// 【教学 Q&A：学生实现后的三个追问】
//
// ── Q1：M1 和 M2 的守卫，本质区别是什么？──
// 学生问："是不是 M1 在参数传 state，函数体内从 state 提取 session 再验证；
//   而 M2 直接在参数提取器提取时就验证了？"
//
// 答：验证逻辑完全一样（都调 extract_token + get_user_by_session），
//   变的是"谁来调用、在哪调用"：
//     M1（命令式）：参数传 State + HeaderMap，函数体内手动 require_user
//       → 保安站在每个房间门口，进门时手动盘查（每个函数写一遍）
//     M2（声明式）：签名写 AuthUser(user): AuthUser，axum 在调用 handler
//       之前自动执行 from_request_parts 完成验证
//       → 楼门禁系统统一刷卡，验证失败（401）根本进不了函数体
//   一句话：M2 不是"验证变快了"，是"验证的位置从函数体搬到了签名"，
//   由框架自动执行，handler 里少写两行重复守卫。
//   但注意：提取器内部还是要查库（session → user），这个"成本"没变，
//   变的只是代码组织方式（封装 + 自动调用）。
//
// ── Q2：extract_token 为什么返回 Option 而不是 Result？──
// 学生问："为什么 extract_token 不直接返回 Result，
//   而是调用时才把 Option 转 Result？"
//
// 答：关键看"没有 token"算不算"错误"：
//     - 没带 cookie / 没有 session=  → None（正常情况：只是没登录而已）
//     - 解析逻辑本身不会失败          → 没有"错误信息"需要携带
//   Option 表达"可能有值，可能没有"，正好够用；
//   Result 的 Err 要携带错误信息，但这里所有失败原因都归为"未登录"，
//   Err 里装什么都是多余——用 Result 反而逼 extract_token 替调用方做决策
//   （"None 算什么错误？"），这是职责越界。
//
//   所以 extract_token 只做"解析"（纯函数：有→Some，无→None），
//   "找不到怎么办"是调用方的事：
//     require_user：ok_or(Unauthorized)?  → 拒绝请求（强制）
//     logout：      if let Some 跳过      → 没活可干（温柔）
//   同一个 None，两种处理，这正是"把决策权留给调用方"的好处。
//   若 extract_token 返回 Result，调用方还得先 match Err 再决定，
//   反而多绕一圈（前面 logout 的注释已经演示过这个对比）。
//
// ── Q3：pub struct AuthUser(pub User); 是什么写法？──
// 学生问："这个写法没见过，是不是结构体再带一个参数进去？
//   为什么不直接放在结构体内部？"
//
// 答：这是"元组结构体"（tuple struct）——结构体名后直接跟括号括住的字段类型，
//   不写字段名。对比三种结构体：
//     struct User { id: i64 }   // 命名结构体：每个字段有名字
//     struct AuthUser(User);    // 元组结构体：字段没名字，只有位置（.0）
//     struct Unit;              // 单元结构体：没有字段
//   构造：AuthUser(user)；解构：AuthUser(u)（模式匹配位置 0）。
//
//   "为什么不写 struct AuthUser { user: User }"？——完全可以！两者等价，
//   只是元组结构体少写一个字段名。AuthUser 只有一个字段且语义明确
//   （"已验证登录的用户"），字段名没得可起，用元组结构体最简洁。
//   单字段包装类型是 Rust 惯例，叫 **newtype 模式**，真正价值是类型安全：
//   把 User 包成 AuthUser，编译器就能区分"已验证的用户"和"普通 User"，
//   防止把没验证的用户当已登录用户用（比如写 user: User 参数，
//   axum 无法区分"这是提取器"还是普通类型，包一层就有了身份）。
//
//   还有一层"孤儿规则"：User 是 sqlx 的查询模型（外部类型），
//   FromRequestParts 是 axum 的 trait（外部 trait）——外部类型不能给
//   外部 trait 实现 impl（孤儿规则限制）。包成我们自己的 AuthUser，
//   就能合法地 impl FromRequestParts<AppState> for AuthUser 了。
// ============================================================

/// 已登录用户（M2 起作为 handler 的"守卫提取器"）
///
/// 用法：handler 签名里写 `AuthUser(user): AuthUser`，
/// 未登录请求会在调用 handler 前被拦截（401 → 重定向登录页）。
pub struct AuthUser(pub User);

impl FromRequestParts<AppState> for AuthUser
{
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection>
    {
        // TODO(M2 第 1 步): 学生实现（步骤见上方注释）
        let token = extract_token(&parts.headers).ok_or_else(|| AppError::Unauthorized)?;
        let user = get_user_by_session(&state.pool, &token).await?;
        Ok(AuthUser(user))
        // unimplemented!("M2 学生实现：AuthUser 提取器")
    }
}

// ============================================================
// 【M5 修订：全局体重维护（POST /profile/weight）】
// 用户问题 0：support 模式的体重应来自"一个地方维护的通用变量"。
// 归属地选 users 表（和 display_name 同款——用户属性）。
// record_form / plan_detail 读取时走 AuthUser（SELECT u.* 已含 body_weight），
// 维护入口在首页"账户"区（每个用户都能改自己的，数据隔离按 user_id）。
// ============================================================
/// 更新自己的全局体重（kg）
///
/// 表单字段：weight（字符串，可空 → 清除体重）
/// 校验：非数字 → Validation 400；负数 → Validation 400；
/// 超范围（> 500kg）→ Validation 400（防手滑）
/// 数据隔离：UPDATE ... WHERE id = ?（只改自己的行）
#[derive(Debug, Deserialize)]
pub struct BodyWeightForm
{
    pub weight: String,
}

pub async fn update_body_weight(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Form(form): Form<BodyWeightForm>,
) -> Result<Redirect, AppError>
{
    // 空串 → 清除体重（None）；否则解析 + 校验
    let weight: Option<f64> = if form.weight.trim().is_empty()
    {
        None
    }
    else
    {
        let w = form
            .weight
            .trim()
            .parse::<f64>()
            .map_err(|_| AppError::Validation("体重必须是数字".to_string()))?;
        if !(0.0..=500.0).contains(&w)
        {
            return Err(AppError::Validation("体重必须在 0~500kg 之间".to_string()));
        }
        Some(w)
    };

    sqlx::query("UPDATE users SET body_weight = ? WHERE id = ?")
        .bind(weight)
        .bind(&user.id)
        .execute(&state.pool)
        .await
        .map_err(AppError::Database)?;

    Ok(Redirect::to("/"))
}
