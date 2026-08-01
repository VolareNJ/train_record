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
mod config;
mod db;
mod error;
mod models;

// 【教学：use 导入】
// use 把其他模块/库的路径引入作用域，避免每次写全路径。
//
// 📌 阶段要求：M0 会"照抄"；M1 起每加新依赖/新模块，能自己补 use。
// 🎯 验收：能解释 use axum::{Router, ...} 里的花括号是"一次导多个"。
use axum::{Router, extract::State, routing::get};
use config::AppConfig;
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
        .nest_service("/static", ServeDir::new("static"))
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
// 【教学：handler（处理器）】
// handler = 处理 HTTP 请求的函数。当浏览器访问对应路径时被调用。
// 签名规则：
//   - async fn
//   - 第一个参数可以是提取器（State/Path/Query/Form/Json...）
//   - 返回类型实现 IntoResponse（String/&str/Html/Json...）
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
async fn home(State(state): State<AppState>) -> String
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
    let user_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users")
        .fetch_one(&state.pool)
        .await
        .unwrap_or(-1);

    // 返回 HTML 字符串
    format!("<h1>训练记录系统</h1><p>M0 脚手架运行成功！</p><p>数据库用户数: {user_count}</p>")
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
