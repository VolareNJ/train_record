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
// ============================================================

// 【教学：模块声明】
// Rust 里每个 .rs 文件是一个"模块"(module)。
// 在 main.rs 里用 mod 关键字声明，编译器才知道有这个文件。
// 注意：模块文件名不带 .rs 后缀。
mod config;
mod db;
mod error;
mod models;

// 【教学：use 导入】
// use 把其他模块/库的路径引入作用域，避免每次写全路径。
use axum::{Router, extract::State, routing::get};
use config::AppConfig;
use sqlx::SqlitePool;
use tower_http::services::ServeDir;

// ============================================================
// 【教学：应用状态 (AppState)】
// 这是 Axum 最重要的概念之一。
//
// 问题：多个 handler（函数）都要用数据库连接池，怎么共享？
// 答案：把共享数据放进一个 struct（这里叫 AppState），
//      在创建 Router 时传进去。之后每个 handler 都能通过
//      `State(state)` 提取器拿到它。
//
// 我们放两个东西：
//   - pool: 数据库连接池（所有 handler 都要查库）
//   - config: 配置（有些 handler 需要，比如备份目录）
// ============================================================
#[derive(Clone)]
pub struct AppState
{
    pub pool: SqlitePool,
    pub config: AppConfig,
}

// ============================================================
// 【教学：#[tokio::main]】
// 这是宏。它把下面的 main 函数包进 tokio 异步运行时里执行。
// 为什么需要？因为 axum 服务器是异步的（同时处理多个请求），
// 必须运行在 tokio 这个"异步运行时"上。
// 初学只需记住：写 axum 项目，main 前加 #[tokio::main]。
// ============================================================
#[tokio::main]
async fn main()
{
    // --------------------------------------------------------
    // 【教学：tracing_subscriber】
    // 日志系统初始化。之后 tracing::info! / tracing::error!
    // 打印的日志会带时间戳和颜色，方便排查问题。
    // --------------------------------------------------------
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    // --------------------------------------------------------
    // 1. 读取配置
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
    let pool = db::init_pool(&config).await.expect("数据库初始化失败");

    // --------------------------------------------------------
    // 3. 组装 AppState
    // --------------------------------------------------------
    // 【教学：SocketAddr】
    // "0.0.0.0" 表示监听所有网卡（这样手机/其他设备都能访问）
    // 端口来自配置。format! 拼出 "0.0.0.0:8080" 字符串，
    // .parse() 转成 SocketAddr 类型。
    // 这里必须显式标注类型，编译器才能推断 .parse() 的目标类型。
    let addr: std::net::SocketAddr = format!("0.0.0.0:{}", config.port)
        .parse()
        .expect("地址解析失败");

    let state = AppState { pool, config };

    // --------------------------------------------------------
    // 4. 构建路由 (Router)
    // --------------------------------------------------------
    // 【教学：Router】
    // Router 是"路径 → 处理函数"的映射表。
    //   .route("/", get(home)) 表示 GET / 时调用 home 函数
    //   .nest_service("/static", ServeDir::new("static"))
    //     表示 /static/xxx 请求去 static/ 目录找文件（CSS/JS）
    //
    // M0 阶段只有一个首页路由，验证服务器能跑通。
    // 后续 M1~M7 在这里逐个加路由。
    let app = Router::new()
        .route("/", get(home))
        .nest_service("/static", ServeDir::new("static"))
        .with_state(state);

    // --------------------------------------------------------
    // 5. 监听端口并启动
    // --------------------------------------------------------
    // axum::serve 启动 HTTP 服务，直到被 Ctrl+C 中断
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("端口绑定失败");
    axum::serve(listener, app).await.expect("服务器运行失败");
}

// ============================================================
// 【教学：handler（处理器）】
// handler = 处理 HTTP 请求的函数。当浏览器访问对应路径时被调用。
// 签名规则：
//   - async fn
//   - 第一个参数可以是提取器（State/Path/Query/Form/Json...）
//   - 返回类型实现 IntoResponse（String/&str/Html/Json...）
//
// 这个 home handler 展示 M0 的成果：
// 首页显示一行欢迎语 + 数据库状态。
// ============================================================
async fn home(State(state): State<AppState>) -> String
{
    // 【教学：State(state) 提取器】
    // 参数里的 State(state) 会自动从请求里取出我们传入的 AppState。
    // 这就是"依赖注入"——handler 需要的共享资源自动拿。

    // 查一下数据库里有多少个用户，验证连接池可用
    // 【教学：sqlx::query_scalar】
    // 查询返回单个值（一个数字）。fetch_one 取第一行。
    // .unwrap_or(-1)：查询失败返回 -1（不至于 panic）
    let user_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users")
        .fetch_one(&state.pool)
        .await
        .unwrap_or(-1);

    // 返回 HTML 字符串
    format!("<h1>训练记录系统</h1><p>M0 脚手架运行成功！</p><p>数据库用户数: {user_count}</p>")
}
