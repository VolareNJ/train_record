// ============================================================
// main.rs —— 程序入口
// ============================================================
// 【教学说明】
// 程序运行顺序（从下往上看更清晰）：
//   1. main() 被操作系统调用
//   2. 读取配置 AppConfig
//   3. 初始化数据库连接池
//   4. 创建 HTTP 路由器（Router），注册路由
//   5. 监听端口，启动服务器
//
// 本文件是"组装车间"：config/db/error/models 都是零件，
// main 把它们组装成一台能跑的服务器。
//
// 📌 本文件各知识点的阶段要求速查（详细见各处注释）：
//   知识            M0         M1          M2+
//   mod 声明        会用        熟练         熟练
//   use 导入        会用        熟练         熟练
//   AppState       理解+会用   会加字段      熟练
//   #[tokio::main] 记住写法    理解一半      熟练
//   日志初始化      了解即可    会写 info!   会分级
//   配置读取        会用        会加字段      熟练
//   连接池          会用        会用          理解内部
//   SocketAddr      了解即可    了解          了解
//   Router 路由     注册 1 个   多个+分组     大量路由
//   handler         返回 String 学提取器     模板+JSON
//   query_scalar    会用        会用          query_as
// ============================================================

// 【教学：模块声明】
// Rust 里每个 .rs 文件是一个"模块"(module)。
// 在 main.rs 里用 mod 关键字声明，编译器才知道有这个文件。
// 注意：模块文件名不带 .rs 后缀。
//
// 📌 阶段要求：M0 会用即可（新写文件要在这里加一行 mod）。
// 🎯 验收：能说出这 4 个 mod 各自对应 src/ 下的哪个文件。
mod auth;
mod config;
mod db;
mod error;
mod handlers;
mod models;

// 【教学：use 导入】
// use 把其他模块/库的路径引入作用域，避免每次写全路径。
//
// 📌 阶段要求：M0 会"照抄"；M1 起每加新依赖/新模块，能自己补 use。
// 🎯 验收：能解释 use axum::{Router, ...} 里的花括号是"一次导多个"。
use axum::{
    Router,
    extract::State,
    http::HeaderMap,
    response::{Html, IntoResponse, Redirect, Response},
    routing::{get, post},
};
use config::AppConfig;
use error::AppError;
use sqlx::SqlitePool;
use tower_http::services::ServeDir;

// ============================================================
// 【教学：应用状态 (AppState)】★ 重点概念，多看几遍
// ============================================================
// AppState 是什么？
//   一句话：它是一个"公共储物柜"，装着所有 handler 都要用的共享数据。
//
// 为什么要它？
//   服务器有多个 handler（处理函数），比如首页、登录、记录页……
//   它们几乎都要查数据库。如果每个 handler 都自己连一次数据库，
//   又慢又乱。正确做法：启动时连一次，装进 AppState，
//   所有 handler 共享同一个连接。
//
// 两个字段分别是什么？
//   - pool: SqlitePool    数据库连接池（"蓄水池"）
//                         所有查库操作都从这拿连接
//   - config: AppConfig   配置（端口/数据库路径/会话密钥）
//                         某些 handler 需要读配置
//
// 【教学：#[derive(Clone)] 是什么意思？】
//   Clone = 让这个 struct 可以被"复制"。
//   为什么要复制？因为 axum 要求 with_state 传入的状态必须能 Clone：
//   每个请求到来时，Router 会 clone 一份 AppState 交给 handler。
//   但别担心"复制很浪费"——
//   SqlitePool 内部是 Arc 智能指针（引用计数），
//   clone 只是把"指向同一个池子的指针"多复制一份，
//   底层还是同一个池子，成本极低，非常安全。
//
// 完整数据流：
//   main() 里创建 AppState { pool, config }
//       → .with_state(state)   挂到 Router 上
//       → 请求到来              axum 自动 clone 一份
//       → handler 写 State(state) 提取器  自动取出
//       → 用 state.pool / state.config 干活
//
// 【教学：AppState ≈ "看得见的全局变量"】
// 说它像"全局变量"——方向对了！它确实是全局共享的：
// 所有 handler 共享同一份数据，生命周期贯穿整个服务器。
// 但它不是真正的全局变量（那种谁都能随手改的）：
//   - 真全局变量  = 钥匙挂公司大门上，人人能拿（易失控、难排查）
//   - AppState    = 前台亲手把钥匙递给你，接了才能用（显式、安全）
// 这种"显式传参"叫【依赖注入】，好处：
//   1. 数据流看得见：main 创建 → with_state → State(state) 接住
//   2. 每个 handler 要什么、拿什么，写在签名里，一目了然
//   3. 测试时能构造假 AppState 传进去，不用碰真的
//
// 📌 阶段要求：
//   M0：理解概念 + 会照抄（知道字段要跟着需求加）
//   M1：自己往 AppState 里加字段（如 session_store）
//   M2+：熟练，能解释为什么 axum 要求 Clone
// 🎯 验收：不看注释，能说出"为什么 AppState 要 #[derive(Clone)]"。
// ============================================================
#[derive(Clone)]
pub struct AppState
{
    /// 数据库连接池：所有 handler 查库都用它
    pub pool: SqlitePool,
    /// 应用配置：端口/数据库路径/会话密钥
    pub config: AppConfig,
}

// ============================================================
// 【教学：#[tokio::main]】
// 这是宏。它把下面的 main 函数包进 tokio 异步运行时里执行。
// 为什么需要？因为 axum 服务器是异步的（同时处理多个请求），
// 必须运行在 tokio 这个"异步运行时"上。
// 初学只需记住：写 axum 项目，main 前加 #[tokio::main]。
//
// 📌 阶段要求：
//   M0：记住"写 axum 项目就要加这一行"即可
//   M1~M2：理解它是"异步运行时"，await 要在这里面才能用
//   M3+：能解释宏展开后做了什么（没必要深究）
// 🎯 验收：看到 async fn + .await，能说出"必须配 #[tokio::main]"。
// ============================================================
#[tokio::main]
async fn main()
{
    // --------------------------------------------------------
    // 【教学：日志初始化】
    // 这三行是"打开日志开关"。
    // 写完之后，代码里任何 tracing::info!("...") 都会在终端打印，
    // 方便你看程序跑到哪一步了（相当于程序自己的"报站"）。
    //
    // 三行各自干什么（现在不用背，用到了再回来看）：
    //   fmt()               = 选择打印格式（带时间戳、带颜色）
    //   with_max_level(INFO) = 只打印 INFO 级及以上的日志（DEBUG 不打印，避免刷屏）
    //   init()              = 生效！从此 tracing 宏都能打印
    //
    // 📌 阶段要求：
    //   M0：了解即可，知道它是"日志开关"
    //   M1：会写 tracing::info! / error! 记录关键事件
    //   M2+：会用不同级别（debug/info/warn/error）区分重要性
    // 🎯 验收：在 home 里加一行 tracing::info!("有人访问首页")，运行后能看到输出。
    // --------------------------------------------------------
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    // --------------------------------------------------------
    // 1. 读取配置
    // 【教学】AppConfig::from_env() 从环境变量读配置，缺失用默认值。
    // 📌 阶段要求：M0 会用；M1 起能自己往 AppConfig 加字段。
    // 🎯 验收：能说出 PORT 和 DATABASE_PATH 两个环境变量分别控制什么。
    // --------------------------------------------------------
    let config = AppConfig::from_env();
    tracing::info!(
        "启动配置: 端口={}, 数据库={}",
        config.port,
        config.database_path
    );

    // --------------------------------------------------------
    // 2. 初始化数据库连接池
    // --------------------------------------------------------
    // 【教学：? 运算符】
    // 如果 init_pool 返回 Err，main 直接返回（程序退出报错）。
    // 数据库连不上是致命错误，启动阶段直接失败比带病运行好。
    // 但注意：main 不能直接用 ?，因为它的返回类型是 ()。
    // 所以用 .expect("...")：出错就 panic 并打印消息。
    //
    // 📌 阶段要求：
    //   M0：会用（理解"连数据库才能查数据"）
    //   M1~M2：会用，知道它返回 SqlitePool（连接池）
    //   M3+：理解连接池内部（为什么比单连接好）
    // 🎯 验收：能说出 .expect 和 ? 的区别（一个 panic，一个向上抛）。
    let pool = db::init_pool(&config).await.expect("数据库初始化失败");

    // --------------------------------------------------------
    // 3. 组装 AppState
    // --------------------------------------------------------
    // 【教学：SocketAddr】
    // "0.0.0.0" 表示监听所有网卡（这样手机/其他设备都能访问）
    // 端口来自配置。format! 拼出 "0.0.0.0:8080" 字符串，
    // .parse() 转成 SocketAddr 类型。
    // 这里必须显式标注类型，编译器才能推断 .parse() 的目标类型。
    //
    // 📌 阶段要求：M0~M2 了解即可（"监听地址"），M3+ 也只需知道"改 IP/端口在这改"。
    // 🎯 验收：能说出 0.0.0.0 和 127.0.0.1 的区别（对外 vs 本机）。
    let addr: std::net::SocketAddr = format!("0.0.0.0:{}", config.port)
        .parse()
        .expect("地址解析失败");

    // 【教学：组装 AppState】
    // 把 pool 和 config 装进"公共储物柜"。之后所有 handler 共享。
    // 📌 阶段要求：M0 会用；M1 加字段时改这里和 struct 定义。
    // 🎯 验收：能说出 state 被 Router 拿去后，handler 怎么拿到它。
    let state = AppState { pool, config };

    // --------------------------------------------------------
    // 3.5 确保管理员存在（M1 新增）
    // --------------------------------------------------------
    // 【教学：首次启动引导】
    // 新系统没有任何用户 → 没人能登录 → 没人能创建用户 → 死锁！
    // 解决方案：环境变量 ADMIN_USERNAME / ADMIN_PASSWORD 指定首个管理员，
    // 启动时若该用户名不存在则自动创建。
    // 若环境变量为空（默认），则跳过（已有用户的系统不会重复创建）。
    //
    // 📌 阶段要求：M1 理解"为什么需要首个管理员"即可。
    // 🎯 验收：能说出没有这段引导会怎样（无法登录 → 无法创建用户）。
    ensure_admin(&state).await;

    // --------------------------------------------------------
    // 4. 构建路由 (Router)
    // --------------------------------------------------------
    // 【教学：Router = "地址 → 函数" 的映射表】
    // 你的理解基本对！router 就是一个"查表器"：
    //   http://IP/     → 运行 home 函数
    //   http://IP/a    → 运行 a 函数（以后加的）
    //   http://IP/b    → 运行 b 函数（以后加的）
    // 注意两个细节：
    //   1. get(home) 里的 get 表示"GET 方法才调"。
    //      同一个地址可以绑多个函数：GET 一个、POST 一个（M1 登录用）。
    //   2. / 后面的路径是"相对路径"，域名/IP 和端口不参与匹配。
    //
    // 【教学：.nest_service = "前缀 → 文件夹"】
    // 它和 .route 不同：不是绑函数，而是绑一个文件夹。
    //   /static/xxx 开头的一切请求 → 去磁盘 static/ 目录找同名文件
    //   例如 /static/style.css → 读取 static/style.css
    //   好处：不用为每个 CSS/JS 文件写函数，一个 ServeDir 全管了。
    //   （M0 时 static/ 是空目录，这行是占位，M6/M7 美化界面时干活）
    //
    // 【教学：.with_state = 把共享数据交给 Router】
    // 这行是"把公共储物柜（AppState）挂到 Router 上"。
    // 流程：
    //   with_state(state)     ← 储物柜挂上 Router
    //   浏览器访问 /          ← 请求进来
    //   axum 查表找到 home    ← 查"地址→函数"映射
    //   axum clone 一份 state ← 因为有这行，才能给
    //   home(State(state))    ← 函数签名接住，就能用 state.pool 了
    // 关键：.with_state 和 handler 参数里的 State(state) 是成对出现的，
    //       缺一个都会编译报错。
    //
    // 📌 阶段要求：
    //   M0：会注册 1 个路由（GET / → home）
    //   M1：会注册多个路由（/login、/register、/logout）
    //   M2+：会 merge 子路由、带路径参数（/exercise/{id}）
    // 🎯 验收：M1 结束时能自己加一个 /hello 路由并访问成功。
    let app = Router::new()
        .route("/", get(home))
        // M1 新增：登录页 + 登录提交（GET 显示表单，POST 处理提交）
        // 【教学：同一路径两个方法 —— "并列 vs 链式"】
        // get() 和 post() 是两个并列的方法处理器，怎么用"链式"写？
        // 关键认知：并列关系体现在【数据结构】里，不在【语法】里。
        //
        // 拆解 get(login_page).post(login) 发生了什么：
        //   1. get(login_page) —— 创建并返回一个 MethodRouter 对象
        //      这个对象内部专门管"哪种 HTTP 方法 → 哪个 handler"的映射
        //      此时对象里只有一项：GET → login_page
        //   2. .post(login) —— 在这个对象上"追加配置"（builder 模式）
        //      往对象里增加一项：POST → login，再返回对象
        //      此时对象里有并列的两项：GET 和 POST 各存一个 handler
        //   3. .route("/login", 那个对象) —— 把"路径 → 方法路由器"绑起来
        //
        // 所以这不是"get 和 post 两个东西链起来"，
        // 而是"先创建对象，再往对象里填第二个字段"——
        // 和你熟悉的 C++ set 链式调用（set_a().set_b()）完全同构。
        //
        // 为什么不用 operator| 或变参函数？
        //   - 变参：Rust 稳定版没有变参泛型，做不到 get(a, b, c)
        //   - operator|：语法可行但可读性差，还容易误看成"位或"
        //   - 方法链：Rust 生态最主流的 builder 写法（tokio/clap 全是）
        //
        // 请求到来时是"两层查表"：
        //   先查路径 /login → 找到这个 MethodRouter
        //   再查方法 POST  → 找到 login 函数
        // 一句话：关系存成结构（并列字段），语法写成串行（逐个填充）。
        .route(
            "/login",
            get(handlers::auth::login_page).post(handlers::auth::login),
        )
        // 登出（POST /logout）
        .route("/logout", post(handlers::auth::logout))
        // 用户管理（仅管理员，守卫在 handler 内部检查）
        .route(
            "/admin/users",
            get(handlers::auth::admin_users).post(handlers::auth::admin_create_user),
        )
        .nest_service("/static", ServeDir::new("static")) // 已有
        .route(
            "/phases",
            get(handlers::phases::list).post(handlers::phases::create),
        )
        .route("/phases/new", get(handlers::phases::create_form))
        .route(
            "/phases/{id}/edit",
            get(handlers::phases::edit_form).post(handlers::phases::update),
        )
        .route("/phases/{id}/archive", post(handlers::phases::archive))
        .route("/phases/{id}/unarchive", post(handlers::phases::unarchive))
        .route(
            "/exercises",
            get(handlers::exercises::list).post(handlers::exercises::create),
        )
        .route("/exercises/new", get(handlers::exercises::create_form))
        .route(
            "/exercises/{id}/edit",
            get(handlers::exercises::edit_form).post(handlers::exercises::update),
        )
        .route("/exercises/{id}/delete", post(handlers::exercises::delete))
        // ----------------------------------------------------------
        // M3 新增：模板（Template）+ 当日计划（Plan）路由
        // 教学注释见 src/handlers/plan.rs 顶部
        // ----------------------------------------------------------
        // 模板：挂在阶段下（/phases/{phase_id}/templates...）
        .route(
            "/phases/{phase_id}/templates",
            get(handlers::plan::list_templates).post(handlers::plan::template_create),
        )
        .route(
            "/phases/{phase_id}/templates/new",
            get(handlers::plan::template_create_form),
        )
        .route(
            "/templates/{id}/edit",
            get(handlers::plan::template_edit_form).post(handlers::plan::template_update),
        )
        .route(
            "/templates/{id}/delete",
            post(handlers::plan::template_delete),
        )
        // 【M4 修订：模板排序】模板上移/下移（?dir=up|down）
        .route("/templates/{id}/sort", post(handlers::plan::template_sort))
        // 【M4 修订：模板项排序】模板内动作上移/下移
        .route(
            "/templates/{id}/items/{item_id}/move",
            post(handlers::plan::template_item_move),
        )
        // 计划：挂在阶段下
        .route(
            "/phases/{phase_id}/plans",
            get(handlers::plan::list_plans).post(handlers::plan::plan_create),
        )
        .route(
            "/phases/{phase_id}/plans/new",
            get(handlers::plan::plan_create_form),
        )
        // 【M4 修订：plan_detail 已并入编辑功能，GET /plans/{id} 直接可编辑；
        // 原 GET /plans/{id}/edit（plan_edit_form）已移除】
        .route("/plans/{id}", get(handlers::plan::plan_detail))
        .route("/plans/{id}/edit", post(handlers::plan::plan_update))
        .route("/plans/{id}/delete", post(handlers::plan::plan_delete))
        // 【M4 修订：计划项排序】计划内动作上移/下移
        .route(
            "/plans/{id}/items/{item_id}/move",
            post(handlers::plan::plan_item_move),
        )
        // ----------------------------------------------------------
        // M4 新增：训练记录（今日页 + 单动作记录/编辑 + 保存）
        // 教学注释见 src/handlers/record.rs 顶部
        // ----------------------------------------------------------
        .route("/today", get(handlers::record::today))
        .route(
            "/plans/{id}/record/{item_id}",
            get(handlers::record::record_form),
        )
        .route(
            "/plans/{id}/record/{item_id}/save",
            post(handlers::record::record_save),
        )
        .with_state(state);

    // --------------------------------------------------------
    // 5. 监听端口并启动
    // --------------------------------------------------------
    // axum::serve 启动 HTTP 服务，直到被 Ctrl+C 中断
    // 📌 阶段要求：M0 了解即可（"启动服务器"），M1+ 不用管它。
    // 🎯 验收：能说出访问首页时，是哪个函数被调用（答 home）。
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("端口绑定失败");
    axum::serve(listener, app).await.expect("服务器运行失败");
}

// ============================================================
// 【教学：ensure_admin —— 首次启动引导】
// 作用：若配置了 ADMIN_USERNAME/ADMIN_PASSWORD，且该用户不存在，
//       自动创建它（密码哈希后入库）。
// 为什么在 main 里调用：main 是程序入口，启动时只执行一次。
// 为什么幂等：先查是否存在，存在就跳过 → 重复启动不会重复创建。
//
// 与 handler 的区别：这个函数不处理 HTTP 请求，只做启动准备。
// 📌 阶段要求：M1 理解"首次启动引导"思路即可，写法可照抄。
// 🎯 验收：能说出如果跳过这步，全新部署会怎样（无人能登录）。
// ============================================================
async fn ensure_admin(state: &AppState)
{
    // 环境变量没配 → 跳过（可能是已有用户的系统）
    if state.config.admin_username.is_empty() || state.config.admin_password.is_empty()
    {
        return;
    }

    // 查该用户名是否存在
    // 【教学：EXISTS 子查询 + query_scalar】
    // SELECT EXISTS(...) 返回 0 或 1（SQLite 里 EXISTS 结果是 0/1）
    // fetch_one 只取第一行第一列 → bool 类型
    let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM users WHERE username = ?)")
        .bind(&state.config.admin_username)
        .fetch_one(&state.pool)
        .await
        .unwrap_or(false);

    // 已存在 → 直接返回（无感静默）
    // 【产品直觉】日志是给"值得关注的事件"用的：
    //   - "创建了管理员"  → 首次部署的信号，值得打日志 ✅
    //   - "已存在，跳过"  → 每次启动都会发生的常态，打日志是噪音 ❌
    // 常态不报站，异常才报站——用户无感进入系统。
    if exists
    {
        return;
    }

    // 不存在 → 创建：哈希密码后插入
    let password_hash =
        crate::auth::hash_password(&state.config.admin_password).expect("密码哈希失败");
    sqlx::query("INSERT INTO users (username, password_hash, is_admin) VALUES (?, ?, ?)")
        .bind(&state.config.admin_username)
        .bind(&password_hash)
        .bind(true)
        .execute(&state.pool)
        .await
        .expect("创建管理员失败");

    tracing::info!("已自动创建管理员: {}", state.config.admin_username);
}

// ============================================================
// 【教学：handler —— 一次请求的完整生命周期】★ 本文件最重要的总览
// ============================================================
// 下面这些概念（Handler/FromRequestParts/State/call/from_request_parts）
// 看起来又多又乱，其实它们是一条链上的不同环节。
// 用 home 当案例，把这条链从头到尾走一遍：
//
//   ┌─────────────────────────────────────────────────────────┐
//   │ ① 注册（编译期）                                          │
//   │   get(home) 被调用                                        │
//   │     → 编译器检查：home 实现 Handler<(State,HeaderMap),   │
//   │         AppState> 吗？                                   │
//   │     → 检查方式：看每个参数类型有没有 FromRequestParts     │
//   │     → 通过 → home 是合法 handler，注册成功                │
//   └─────────────────────────────────────────────────────────┘
//                                ↓
//   ┌─────────────────────────────────────────────────────────┐
//   │ ② 请求到达（运行时）                                      │
//   │   浏览器访问 /                                            │
//   │     → Router 查路径表 → 找到 home                        │
//   │     → axum 调 Handler::call(home, req, state)            │
//   └─────────────────────────────────────────────────────────┘
//                                ↓
//   ┌─────────────────────────────────────────────────────────┐
//   │ ③ 提取参数（call 内部，宏展开的顺序语句）                   │
//   │   t1 = State<AppState>::from_request_parts(...)          │
//   │   t2 = HeaderMap::from_request_parts(...)                │
//   │      （每参数一行，不是循环）                               │
//   └─────────────────────────────────────────────────────────┘
//                                ↓
//   ┌─────────────────────────────────────────────────────────┐
//   │ ④ 真正调用你的函数                                        │
//   │   home(t1, t2).await → 你的守卫、查库、返回 HTML          │
//   │     返回值再 .into_response() 转成 HTTP 响应              │
//   └─────────────────────────────────────────────────────────┘
//
// 记住两个 trait 的分工，整条链就通了：
//   - Handler<T, S>    ："这个函数能当 handler 吗？"（外层包装）
//   - FromRequestParts： "怎么从请求里造出一个参数？"（内层零件）
//
// ============================================================
// 【教学①：编译期检查 —— 怎么"知道" handler 的签名】
// ============================================================
// get(home) 传的不是函数指针，是函数本身（函数项）。get 是泛型的：
//   pub fn get<H, T, S>(handler: H) -> MethodRouter<S>
//   where H: Handler<T, S>          // ★ 关键约束
// 而 Handler trait 对"任意函数"自动生效（blanket impl）：
//   impl<F, Fut, Args, Res, S> Handler<Args, S> for F
//   where
//       Args: FromRequestParts<S> + Send,   // ① 每个参数都是合法提取器
//       Fut: Future<Output = Res>,
//       Res: IntoResponse,                  // ② 返回值能转 HTTP 响应
// 所以"怎么知道签名"的答案是：编译期静态推断，不是运行时查表。
//   签名不合格 → 编译直接报错，程序根本跑不起来。
//
// 请求到来时，axum 按参数【顺序】逐个调用提取器，把结果依次传进函数。
// 所以 handler 参数可以任意组合：State、HeaderMap、Form、Query...
// 只看"类型 + 顺序"，不管具体是啥。
//
// ============================================================
// 【教学②：FromRequestParts —— 提取器的"许可证"】
// ============================================================
// 一个类型想当 handler 参数，就必须实现 FromRequestParts，
// 并告诉 axum："怎么从请求里把我造出来"。它是统一的提取器契约。
// 定义（简化）：
//   trait FromRequestParts<S>: Sized {
//       type Rejection: IntoResponse;  // 提取失败时返回什么（如 401）
//       fn from_request_parts(
//           parts: &mut Parts,  // 方法/URI/请求头（Cookie 就在 headers 里！）
//           state: &S,          // AppState 的引用（连接池就在这！）
//       ) -> impl Future<Output = Result<Self, Self::Rejection>> + Send;
//   }
// from_request_parts 就是这个 trait 的【唯一方法】——"提取动作本身"。
// 它拿到 parts（请求头）和 state（AppState），就能造出自己。
// 这正是未来 M2+ 写 AuthUser 所需的全部材料：
//   读 headers 里的 cookie → 查 session → 返回 User。
//
// ============================================================
// 【教学③：State —— "装值盒子" + blanket impl】
// ============================================================
//   pub struct State<S>(pub S);   // 就这一行，一个包着 S 类型值的空壳
// 它不是枚举（没有变体、不用 match）。存在意义 = 类型标记：
//   让类型系统能区分"这是状态提取器"和"这只是个 AppState 值"。
//
// 为什么 AppState 不需要标 FromRequestParts？
//   因为限界标记根本不在 AppState 身上，而在 State 身上！
//   axum 内置了 blanket 实现（对任意类型 S 都成立）：
//     impl<S> FromRequestParts<S> for State<S>
//     where S: Clone + Send + Sync {
//         fn from_request_parts(parts, state) -> ... {
//             Ok(State(state.clone()))   // 提取动作就一句：clone 装盒
//         }
//     }
//   AppState 只是恰好填进泛型 S 这个"坑位"。
//   这也解释了为什么 AppState 必须 #[derive(Clone)]：
//     上面那句 state.clone() 就是 main.rs 那段
//     "axum 要求 Clone" 注释的源码出处。
//
// 为什么 get 不直接要求参数就是 State？
//   因为 handler 的参数类型不只有 State 一种：
//   HeaderMap / Path / Query / Form 以及未来的 AuthUser
//   全都实现同一个 FromRequestParts trait。
//   泛型 + trait 限界是【统一机制】：不关心你具体是啥，
//   只要你说得清"怎么从请求里造出我"就能当参数。
//   这就是开放设计：任何人实现这个 trait 就能用。
//
// ============================================================
// 【教学④：Handler::call —— 宏展开 + 单态化（性能上零开销）】
// ============================================================
// 问：get 是不是变参模板？内部是不是维护 vtable 装 &dyn Trait，
//      然后 home(vtable[0], vtable[1]).await？会不会有虚调用开销？
// 答：三层拆解。
//
// ① get 不是变参 —— Rust 没有变参泛型（做不到 get(a,b,c) 任意个数）。
//    axum 的解法：Handler trait 的第二个泛型参数 T 是一个【元组】，
//    元组长度 = 参数个数。再用宏批量生成"每个长度各一份实现"：
//      impl Handler<(T1,), S> for F           // 1 参数版本
//      impl Handler<(T1, T2), S> for F        // 2 参数版本
//      impl Handler<(T1, T2, T3), S> for F    // 3 参数版本
//      ... 一直到 64 个参数
//    相当于 C++ 手写 64 个重载，只是用宏代劳了。
//
// ② call 内部不是循环 —— 是宏展开出来的【顺序语句】：
//      let t1 = T1::from_request_parts(&mut parts, &state).await?;
//      let t2 = T2::from_request_parts(&mut parts, &state).await?;
//      let res = self(t1, t2).await;   // 调用你写的 home
//    编译时 T1/T2 都是具体类型（State<AppState>、HeaderMap），
//    所以 T1::from_request_parts(...) 是直接调用，没有指针跳转。
//    这就是【单态化】（monomorphization）——Rust 版的模板实例化。
//
// ③ vtable 直觉对了一半，但位置不对：
//    - 参数提取层：纯静态分发，零虚调用，编译期内联。
//    - 路由存储层：Router 要同时装 home/login/logout...它们类型各不同，
//      必须类型擦除成 Box<dyn Service> 才能放进同一个路由表
//      ——这才是你说的 vtable/&dyn Trait。每请求【一次】装箱间接调用，
//      固定成本，相对网络 IO / 数据库查询可以忽略不计。
//
// 和 C++ 的对照：
//   C++ 变参模板(parameter pack)  →  Rust 元组 + 宏展开
//   C++ 模板实例化                 →  Rust 单态化 (monomorphization)
//   C++ std::function/vtable       →  Rust Box<dyn Trait> 类型擦除
// 一句话：axum 把"变参"翻译成"元组+宏"，提取层全静态，只在路由层
// 做一次装箱——这是所有 web 框架都躲不掉的最小代价。
//
// ============================================================
// 【教学④·补充：axum 内部的两层结构 —— 函数如何"变成"handler】
// ============================================================
// 学生追问：为什么没看到 impl Handler for home？trait 不是只能
//   impl 给 struct 吗？是不是有个 Handler"类"包装了函数？
//   答：分两层（这层理解透，axum 内核就通了）。
//
// ── 第一层：trait 直接写在函数类型上（编译期，零包装）───────
//   Rust 的 trait 可以 impl 给【任何类型】，包括函数本身：
//   每个函数都有一个唯一的"函数项类型"（匿名、零大小、不占内存），
//   它实现了 FnOnce(Args) -> Fut —— "能被调用，参数是 Args"。
//   然后 blanket impl（一揽子实现）自动覆盖这种类型：
//     impl<F, Fut, Args, Res, S> Handler<Args, S> for F
//     where
//         F: FnOnce(Args) -> Fut + Clone + Send + 'static,
//         Args: FromRequestParts<S> + Send,
//         Fut: Future<Output = Res> + Send,
//         Res: IntoResponse,
//   编译器见到 get(home) 时自动匹配，无需手写一行 impl。
//   （这就是 Rust 和 C++ 的根本差异：方法不是只能加在 class 上）
//
// ── 第二层：HandlerService 才是那个"包装"（≈ std::function）──
//   路由表要求所有东西都是统一的 Box<dyn Service>，但 home 的
//   类型很特殊 → axum 用 HandlerService::new(home) 包一层，
//   使它实现 tower::Service，Router 才能存它、调它。
//   与 std::function 的区别：std::function 是运行时类型擦除
//   （堆分配 + 虚调用）；HandlerService 是编译期泛型（单态化）。
//
//   C++ 对照表：
//     Handler trait  = C++ concepts（模板约束）
//     HandlerService = std::function 的角色（但零擦除开销）
//     函数项类型     = 模板实参的具体类型（F，不是 std::function）
//
// ── 完整调用链（编译期 → 运行期）───────────────────────────
//   编译期：get(home)
//     → blanket impl 自动判定 home 实现 Handler
//     → axum 用 HandlerService::new(home) 包成 Service
//     → 存入路由表（类型擦除成 Box<dyn Service>）
//   运行期：请求 → Router 查路径表 → 命中那个 Service 的 call
//     → 内部调 <home 类型 as Handler>::call(req, state)
//     → call 里 let (mut parts, body) = req.into_parts()
//     → 逐个 T::from_request_parts(&mut parts, &state).await?
//     → let res = self(t1, t2).await     // 调用你的 home
//     → res.into_response()
//   注意：get() 只在编译期出现一次；运行期入口是路由表里
//   那个 Service 的 call，不是"get 方法"。
//
// ── 关键纠正：from_request_parts 是"构造"，不是"剥包装" ────
//   它返回的就是 Self 本身（带着包装）：
//     State::<AppState>::from_request_parts(...) → State<AppState>
//     HeaderMap::from_request_parts(...)         → HeaderMap
//   home 的参数类型（State<AppState>、HeaderMap）与提取器返回值
//   完全一致，call 里直接传进去，中间没有任何"拆开再传"。
//   State(state) 是【参数模式】：home 被调用的瞬间自动解构盒子，
//   把 AppState 绑定到 state —— 等价于 C++ 的
//   auto [a, b] = p;，只是把解构从函数体提前到了参数位置。
//
// ── parts 是什么？──────────────────────────────────────────
//   req.into_parts() 把请求拆成 (Parts, Body)：
//     Parts = 方法 / URI / 请求头（Cookie 在这）/ 扩展数据
//     Body  = 请求体（只有 Form/Json 这类提取器才需要）
//   各提取器各取所需：HeaderMap → 读 headers；Path/Query → 解析 uri；
//   State → 两个都不读，只要 state 参数（clone 装盒）。
//   为什么 parts 是 &mut？因为 Extension 会从 parts 里【取走】东西
//   （所有权转移）。为什么分两个 trait？Form/Json 需要整个请求
//   （FromRequest），State/HeaderMap 只需头部（FromRequestParts）。
//
// ── 抽象链：从"通用框架"到"你的代码"，每层一种抽象手段 ──────
//
//   浏览器
//     │  HTTP 请求（字节流）
//     ▼
//   [Router 路由表]              抽象①：类型擦除（运行时多态）
//   装 Box<dyn Service>          home/login/logout 类型各异，
//     │  match 路径 → 命中目标      统一擦成"Service"才能同表存放
//     ▼                           （vtable，每请求 1 次间接调用）
//   [HandlerService<H, T, S>]   抽象②：泛型适配（≈ std::function）
//   实现 tower::Service          包一层让"特殊类型"也能当 Service
//     │  Service::call(req, state) 编译期泛型，零虚调用
//     ▼
//   [Handler trait]             抽象③：blanket impl
//   <home类型 as Handler>::call  函数项类型自动获得"能当 handler"
//     │                             的资格，不需要手写 impl
//     ▼
//   [宏展开 + 提取器]           抽象④：统一契约（构造 Self）
//   T1::from_request_parts(&mut parts, &state).await?   ← 宏逐参数展开
//   T2::from_request_parts(&mut parts, &state).await?   State → State<AppState>
//     │  每参数一行，编译期单态化                            HeaderMap → HeaderMap
//     ▼
//   [你的函数 home]             抽象⑤：具体业务代码（无抽象）
//   State(state) 参数模式解构    守卫 → 查库 → 返回 Html / Redirect
//     │
//     ▼
//   [Response] → 回浏览器
//
//   抽象手段沿链路递减：
//     类型擦除(vtable) → 泛型适配 → trait + blanket impl
//     → 宏展开(单态化) → 具体函数
//   越往上越通用（框架层替你兜底），越往下越具体（你的代码）。
//
// ============================================================
// 【教学⑤：impl Future / impl Trait —— 返回"匿名类型"】
// ============================================================
//   - 普通写法 fn f() -> SomeConcreteType：必须写死一个具体类型。
//   - impl Trait 写法 fn f() -> impl Future<...>：不写具体类型，
//     只承诺"返回的东西实现了 Future trait"。
//     调用方知道"它是 Future，可以 .await"，但不用知道它具体叫什么。
//   为什么这里必须用 impl Trait？因为 from_request_parts 内部是
//     async 块（async {} 或 .await 链），async 块编译后会生成一个
//     【编译器才知道名字的匿名类型】——人没法写出它的名字，
//     所以用 impl Future 说"就是个 Future，能 await 就行"。
//   注意：impl Trait 只隐藏"具体是哪个类型"，不隐藏 trait 能力。
//   这比 C++ 的 auto 更严格：auto 随便什么都能推导，impl Trait
//   必须满足指定的 trait，否则编译不过。
//
// ============================================================
// 【教学⑥：实战易错点 —— 两个"坑"（都是学生踩过的）】
// ============================================================
// 坑 1：在返回 String 的 handler 里用 ? 报错
//   ? 的意思是"如果是 Err，就 return 这个错误"。
//   但 String 没有"错误通道"——? 想 return 的 AppError 装不进去。
//   解决：返回类型改成 Result<Response, AppError>（或 Result<String, ...>）。
//
// 坑 2：.into_response() 报"method not found"
//   into_response 是 trait 方法，必须把 IntoResponse 也 use 进来：
//     use axum::response::{Html, IntoResponse, Redirect, Response};
//   （只导入类型不够，trait 本身也要在作用域内，类似 Iterator）
//
// 为什么 home 返回 Result<Response, AppError> 而不是 Result<String, ...>？
//   因为成功分支有两种返回值：HTML 字符串 / 重定向 Redirect。
//   它们类型不同，String 装不下，只有 Response（"通用响应"）能装下。
//   这两种值都实现 IntoResponse，用 .into_response() 转成统一类型。
//
// 这个 home handler 展示 M0 的成果：
// 首页显示一行欢迎语 + 数据库状态。
//
// 📌 阶段要求：
//   M0：会写"返回 String"的简单 handler
//   M1：学 Form/Query 提取器（登录表单）、返回重定向
//   M2+：返回 askama 模板（Html）、处理 JSON
// 🎯 验收：M0 结束能自己新写一个 handler 并注册路由。
// ============================================================
async fn home(State(state): State<AppState>, headers: HeaderMap) -> Result<Response, AppError>
{
    // 【教学：State(state) 提取器】
    // 参数里的 State(state) 会自动从请求里取出我们传入的 AppState。
    // 这就是"依赖注入"——handler 需要的共享资源自动拿。
    // 📌 阶段要求：M0 会用（照抄）；M1 理解"每个 handler 都能拿 state"。
    // 🎯 验收：能说出 state.pool 是什么（数据库连接池）。

    // 查一下数据库里有多少个用户，验证连接池可用
    // 【教学：sqlx::query_scalar】
    // 查询返回单个值（一个数字）。fetch_one 取第一行。
    // .unwrap_or(-1)：查询失败返回 -1（不至于 panic）
    //
    // 📌 阶段要求：
    //   M0：会用 query_scalar 查单个数字
    //   M1：会用 query_as + FromRow 查整行转 struct
    //   M2+：会用 query! 宏（编译期检查 SQL）
    // 🎯 验收：能说出 fetch_one 和 fetch_all 的区别（一行 vs 多行）。

    // 未登录 → 重定向到登录页；其他错误 → 原样返回
    let user = match handlers::auth::require_user(&state, &headers).await
    {
        Ok(user) => user,
        Err(AppError::Unauthorized) => return Ok(Redirect::to("/login").into_response()),
        Err(e) => return Err(e),
    };

    // 查询统计数字（页面卡片展示用）
    // 【教学：跨表统计 —— "我的阶段下的模板/计划"怎么数？】
    // 模板/计划表没有 user_id 列（它们挂在 phase 下），不能直接按 user 过滤。
    // 解法是【子查询】先圈出"当前用户的阶段"，再数这些阶段下的模板/计划：
    //   SELECT COUNT(*) FROM templates
    //   WHERE phase_id IN (SELECT id FROM phases WHERE user_id = ?)
    // 这就是"跨表统计"的入门形态：一层查询套一层查询。
    let phase_count = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM phases WHERE user_id = ? AND archived = 0",
    )
    .bind(&user.id)
    .fetch_one(&state.pool)
    .await
    .map_err(AppError::Database)?;
    let exercise_count =
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM exercises WHERE user_id = ?")
            .bind(&user.id)
            .fetch_one(&state.pool)
            .await
            .map_err(AppError::Database)?;
    let template_count = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM templates
    WHERE phase_id IN (SELECT id FROM phases WHERE user_id = ?)",
    )
    .bind(&user.id)
    .fetch_one(&state.pool)
    .await
    .map_err(AppError::Database)?;
    let plan_count = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM plans
    WHERE phase_id IN (SELECT id FROM phases WHERE user_id = ?)",
    )
    .bind(&user.id)
    .fetch_one(&state.pool)
    .await
    .map_err(AppError::Database)?;

    // 管理员专属入口（首页只对管理员显示"用户管理"链接）
    let admin_link = if user.is_admin
    {
        r#"<a href="/admin/users">用户管理</a>"#
    }
    else
    {
        ""
    };

    // 【教学：首页的"进行中阶段"直达入口】
    // 模板/计划都挂在阶段下，列表页路由是 /phases/{phase_id}/templates 和 /phases/{phase_id}/plans。
    // 用户最常操作的阶段是"进行中"的那个（未归档、最新创建），
    // 首页直接把它找出来，给出模板/计划的直达链接，少点一层。
    // 没有进行中阶段 → 显示"先去创建阶段"的引导链接。
    let current_phase = sqlx::query_as::<_, crate::models::Phase>(
        "SELECT * FROM phases WHERE user_id = ? AND archived = 0 ORDER BY created_at DESC LIMIT 1",
    )
    .bind(&user.id)
    .fetch_optional(&state.pool)
    .await
    .map_err(AppError::Database)?;
    let phase_links = match &current_phase
    {
        Some(phase) => format!(
            r#"<li>当前阶段：<a href="/phases/{phase_id}/templates">训练模板</a> | <a href="/phases/{phase_id}/plans">训练计划</a></li>"#,
            phase_id = phase.id,
        ),
        None => r#"<li><a href="/phases/new">先去创建训练阶段</a></li>"#.to_string(),
    };

    // 返回 HTML 字符串
    // 【教学：首页导航的"分区"设计】
    // 首页是导航中枢，入口按"功能归属"分区展示：
    //   训练管理：今日训练（记录）、当前阶段的模板/计划直达、阶段、动作库
    //   账户：用户管理（管理员）、登出
    // 排版上用 <section> 分区 + <li> 列表，比平铺的一排链接清晰。
    // 模板/计划同时保留在阶段列表每行里（见 phases.rs list 的注释）——
    // 首页只放"当前进行中阶段"的直达入口，其余阶段仍从阶段列表进入，避免首页堆满链接。
    Ok(Html(format!(
        r#"<head><meta name="viewport" content="width=device-width, initial-scale=1.0"></head>
        <h1>训练记录系统</h1>
        <p>欢迎回来，{username}！</p>
        <h2>数据概览</h2>
        <p>进行中阶段：{phase_count} 个</p>
        <p>动作库：{exercise_count} 个</p>
        <p>训练模板：{template_count} 个</p>
        <p>训练计划：{plan_count} 个</p>
        <h2>训练管理</h2>
        <ul>
            <li><a href="/today">今日训练（记录）</a></li>
            {phase_links}
            <li><a href="/phases">查看训练阶段（含模板 / 计划）</a></li>
            <li><a href="/exercises">查看训练动作</a></li>
        </ul>
        <h2>账户</h2>
        <ul>
            {admin_link}
            <li><form method="post" action="/logout" style="display:inline"><button type="submit">登出</button></form></li>
        </ul>
        "#,
        username = user.username,
        phase_count = phase_count,
        exercise_count = exercise_count,
        template_count = template_count,
        plan_count = plan_count,
        phase_links = phase_links,
        admin_link = admin_link,
    ))
    .into_response())
}

// ============================================================
// 【练习回顾：M0 三小练习（已验收，代码已回退，记录于此供复习）】
// ============================================================
// 1. 改首页欢迎文字 ✅
//    做法：在 format! 里加了一段 <p>Hello World!</p>
//    收获：Rust 字符串字面量可跨行，换行会保留为 \n，浏览器渲染为空格。
//    回退原因：保持 M0 脚手架原样，避免影响正式页面。
//
// 2. 加 phases 计数 ✅
//    做法：模仿 user_count 再加一个查询：
//      let phase_num: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM phases")
//          .fetch_one(&state.pool).await.unwrap_or(-1);
//    收获：query_scalar 查单值 + fetch_one 取一行，与 user_count 完全同构。
//    回退原因：同上，避免与 M0 定义代码混淆。
//
// 3. 端口 8080 → 3000 ✅
//    做法：config.rs 里 unwrap_or_else(|| "8080".to_string()) 改为 "3000"
//    收获：理解了"配置 → 生效"链路：config.rs 的默认值 → AppConfig → main.rs 绑定端口。
//    回退原因：README/文档均以 8080 为准，保持默认一致。
//
// 【理解验证 3 题：2 题通过，第 3 题方向对但缺关键机制】
//   第 1 题 AppState：✅ 理解为"handler 共用的储物柜，提供数据库池访问"
//   第 2 题 连接池：   ✅ 理解为"复用连接避免反复创建销毁的开销 + 排队机制"
//   第 3 题 迁移幂等：⚠️ 答了"表已建过"，但关键机制是——
//                     sqlx 把已执行记录写进数据库里的 _sqlx_migrations 表，
//                     下次启动先查该表，已执行的跳过、没执行的才执行。
//                     （类比：不是靠记忆，而是靠"已办事项清单"记账）
// ============================================================
