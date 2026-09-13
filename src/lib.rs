// ============================================================
// lib.rs —— 库入口（模块树 + 共享状态）
// ============================================================
// 【教学说明：为什么项目里同时有 lib.rs 和 main.rs？】
//
// 一个 Cargo 包可以同时提供两种"编译目标"（target）：
//   · lib  （src/lib.rs）  → 库，别人（包括同包的 bin、以及 tests/、examples/）可以引用
//   · bin  （src/main.rs） → 可执行程序，必须有 main()，链接同一个包里的 lib
//
// 本项目到 M9 为止的实际情况（为什么要拆开）：
//   1. **模块树只能有一处**：以前 9 个 `mod xxx;` 写在 main.rs 里，意味着
//      "这些模块属于 bin 目标"。想给 tests/（集成测试）或未来的工具引用就做不到。
//      移到 lib.rs 后：模块属于**库**，bin 通过 `train_record::...` 引用它们。
//   2. **测试目标更清晰**：cargo test 会分别跑 lib 与 bin 的测试；
//      AppState / db / api 这些都在 lib 里，测试不必再依赖 bin 的私有结构。
//   3. **未来可复用**：M10 的 iced 客户端是另一个 crate；如果哪天想把
//      calc.rs（1RM 纯函数）或 api 的类型做成共享库，出口已经现成。
//
// 拆分后的分工（一句话）：
//   lib.rs  = "应用本体"（模块树 + 共享状态 AppState），能被测试/其他目标引用
//   main.rs = "启动脚本"（读配置 → 起服务器 → 组装路由），只做组装与启动
//
// 【教学：为什么模块都写 `pub mod`？】
// lib 里的东西默认对外不可见（`mod xxx;` = 私有模块）。
// bin 要用 `train_record::handlers::...` 引用它们，所以至少要 pub 到"包外可见"。
// 本项目全 pub 是为了简单 + 教学（这也是个取舍）：
//   真实库应该只 pub 真正想承诺的 API，其余用 pub(crate) 收窄
//   （比如 calc 只在内部用，可以 `pub(crate) mod calc;`）。
//   本项目的定位是"一个可执行程序 + 一点点共享代码"，所以不刻意收窄。
//
// 【全路径约定（M9 起）】本文件同样遵守：类型/函数写全路径，
// 只有 trait 方法调用才 import 那一个 trait（见 AGENTS.md）。
// ============================================================

// 【教学：模块声明】
// Rust 里每个 .rs 文件是一个"模块"(module)。
// 在**包根**（lib.rs 或 main.rs）里用 mod 关键字声明，编译器才知道有这个文件。
// 注意：模块文件名不带 .rs 后缀；层级目录（api/rest/...）由各目录的 mod.rs 汇总。
//
// 阶段要求：M0 会用即可（新写文件要在这里加一行 mod）。
// 验收：能说出这 9 个 mod 各自对应 src/ 下的哪个文件/目录。
pub mod api; // 给程序用的两个出口：rest（JSON）/ grpc（protobuf）
pub mod auth; // 逻辑层：密码哈希、session 增删查（不碰 HTTP）
pub mod calc; // 纯函数：1RM（Epley/Wathan）等计算，带单元测试
pub mod config; // 配置：端口 / GRPC_PORT / 数据库路径 / 密钥 / 部位顺序
pub mod db; // SQLite 连接池 + 迁移
pub mod error; // 页面层统一错误类型（HTTP 语义：302/422/500）
pub mod handlers; // 页面层（SSR）：查库 + 拼 HTML
pub mod models; // 领域模型（与数据库表一一对应）
pub mod page; // 页面公共片段（如 page_head）

// ============================================================
// 【教学：应用状态 (AppState)】重点概念，多看几遍
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
// 【教学：为什么它定义在 lib.rs 而不是 main.rs？】（M9 结构调整）
//   AppState 被 handlers/、api/、以及测试（api/grpc/smoke_test.rs）使用，
//   都是"库"里的代码。定义在 main.rs（bin）里的话，它们属于两个不同编译目标，
//   库根本引用不到 bin 里的类型。所以共享类型必须住在库里。
//
// 【教学：#[derive(Clone)] 是什么意思？】
//   Clone = 让这个 struct 可以被"复制"。
//   为什么要复制？因为 axum / tonic 都要求注册的状态必须能 Clone：
//   每个请求到来时，框架会 clone 一份 AppState 交给 handler。
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
//   （gRPC 出口同理：每个 service 实现里也持有一份 AppState，见 api/grpc/server.rs）
//
// 【教学：AppState 约等于"看得见的全局变量"】
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
// 阶段要求：
//   M0：理解概念 + 会照抄（知道字段要跟着需求加）
//   M1：自己往 AppState 里加字段（如 session_store）
//   M2+：熟练，能解释为什么 axum 要求 Clone
// 验收：不看注释，能说出"为什么 AppState 要 #[derive(Clone)]"。
// ============================================================
#[derive(Clone)]
pub struct AppState
{
    /// 数据库连接池：所有 handler 查库都用它
    /// （Arc + RwLock 是为了"备份上传后免重启热替换"，见 M7）
    pub pool: std::sync::Arc<tokio::sync::RwLock<sqlx::SqlitePool>>,
    /// 应用配置：端口/gRPC 端口/数据库路径/会话密钥
    pub config: crate::config::AppConfig,
}
