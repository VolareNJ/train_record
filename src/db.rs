// ============================================================
// db.rs —— 数据库连接池与初始化模块
// ============================================================
// 【教学说明】
// 这个模块负责两件事：
//   1. 创建【连接池】(Connection Pool) —— 数据库连接的"蓄水池"
//   2. 运行【迁移】(Migration) —— 自动创建表结构
//
// 为什么需要连接池？
//   Web 服务器同时处理多个请求（比如手机和电脑同时访问）。
//   如果每个请求都重新打开数据库文件，性能差且容易冲突。
//   连接池维护一批已打开的连接，请求来了取一个用，用完还回去。
//   本项目虽然数据量小，但这是标准做法，一开始就做对。
//
// 连接池类型：sqlx::SqlitePool
//   它是 Arc 内部共享的，可以 clone 一份传给多个 handler 使用。
// ============================================================

use sqlx::{
    ConnectOptions, SqlitePool,
    sqlite::{SqliteConnectOptions, SqlitePoolOptions},
};
use std::str::FromStr;

use crate::config::AppConfig;

/// 初始化数据库连接池
///
/// 【教学说明】
/// - 参数 db_path: 数据库文件路径（来自 config）
/// - 返回值: Result<SqlitePool, sqlx::Error>
/// - async 函数：因为打开数据库是 I/O 操作，需要 await
/// - 【重要】每次启动都调用（不是只第一次）！
///   因为程序运行期间每个请求都要查数据库，连接池必须从一开始就存在。
///
/// 步骤：
///   1. SqliteConnectOptions：配置如何连接（文件路径 + 创建缺失文件）
///   2. SqlitePoolOptions：配置连接池大小
///   3. .connect()：真正建立连接池
///   4. run_migrations()：确保表结构存在（幂等，已执行过的不重复执行）
pub async fn init_pool(config: &AppConfig) -> Result<SqlitePool, sqlx::Error>
{
    // 【教学：Builder 模式】
    // Rust 生态常用"链式调用"构造配置：每个方法返回自身，可连写
    let connect_options = SqliteConnectOptions::from_str(&config.database_path)?
        // create_if_missing(true)：文件不存在就自动创建
        .create_if_missing(true)
        // 启用外键约束（我们表之间有 FOREIGN KEY 关联）
        //
        // 【教学：外键约束是干什么的？】
        // 背景：我们的数据表不是孤立的，它们互相关联。
        // 比如 phases（训练阶段）表里有一列 user_id，记录"这个阶段属于哪个用户"。
        // 这个 user_id 不是随便填的数字，它必须指向 users 表里真实存在的一行。
        //
        // 外键约束 = 数据库帮我们检查这个"指向关系"的规则，防止脏数据：
        //   1. 防孤儿：往 phases 插入 user_id=999 时，如果 users 表里没有 999 号用户，
        //      数据库直接报错拒绝（不能插入"没有主人的数据"）
        //   2. 防误删：删除某个用户时，如果他的阶段数据还在，数据库也会拦着，
        //      避免留下指向不存在用户的孤儿记录
        //
        // 类比：外键就像"身份证号"——你去办业务时报的身份证号，系统会
        // 查公安系统确认这个人真的存在。外键约束就是让数据库替我们做这个检查。
        //
        // 本项目的外键关系（见 migrations/0001_init.sql）：
        //   phases.user_id      -> users.id       （阶段属于某个用户）
        //   exercises.user_id   -> users.id       （动作属于某个用户）
        //   templates.phase_id  -> phases.id      （模板绑定某个阶段）
        //   template_items.template_id -> templates.id（模板动作项属于某个模板）
        //   plans / plan_items / records 同理，都指向各自的"主人"
        //
        // 注意：SQLite 默认【不】启用外键检查！必须写 .foreign_keys(true)
        // 显式打开，否则上面这些约束形同虚设。这就是这一行的意义。
        .foreign_keys(true)
        // 日志级别：连接失败等才打印，避免刷屏
        .log_statements(log::LevelFilter::Debug);

    // 连接池配置
    // 【教学：这 5 条连接是"程序↔数据库"的连接，不是"用户↔程序"的连接】
    // 用户浏览器走的是 HTTP 连接（每次请求一条，用完即断），
    // 而池里的连接是程序与 SQLite 文件之间共享的数据库连接：
    //   浏览器 ─HTTP─▶ 程序(handler) ─借连接─▶ 池(最多5条) ─▶ SQLite 文件
    // 请求来了从池里借一条，用完归还，所有请求共用这最多 5 条。
    // 为什么 5 条够用？SQLite 是单文件，同一时刻只有一个连接能写，
    // 开再多写操作也得排队，5 条绰绰有余。
    let pool = SqlitePoolOptions::new()
        // 最大连接数。SQLite 单文件，并发写需要排队，5 个够用
        .max_connections(5)
        // 连接空闲超时
        // 【教学：idle_timeout 是"空闲"超时，不是"等待"超时！】
        // 含义：连接闲置 60 秒没人用 → 池子关掉它 → 释放系统资源（文件句柄、内存）
        // 注意：程序启动时池里是 0 条连接，不是立刻开 5 条！
        // 是"有请求才开（懒创建）、闲久了就关（动态伸缩）"。
        // 类比：外卖店最多雇 5 个外卖员，闲了 60 秒就让他下班省开支，忙了再招。
        // 为什么？数据库连接是贵资源，常年闲着是浪费，这是性能与资源的平衡。
        .idle_timeout(std::time::Duration::from_secs(60))
        // 用上面的连接配置建立连接池
        .connect_with(connect_options)
        .await?;

    // 运行数据库迁移（创建表结构）
    run_migrations(&pool).await?;

    Ok(pool)
}

/// 运行数据库迁移
///
/// 【教学说明】
/// sqlx 的 migrate! 宏会在编译时读取 migrations/ 目录下的 .sql 文件，
/// 按文件名顺序执行，并且只在表不存在时创建（幂等）。
///
/// 【教学：为什么每次启动都跑 run_migrations？——幂等 + 自动升级】
/// 迁移是幂等的：跑多少遍结果都一样。
///   - 第一次启动：_sqlx_migrations 表为空 → 执行 0001_init.sql → 建表 → 记录"已执行"
///   - 之后每次启动：对比发现已执行 → 全部跳过 → 几乎零成本
/// 真正的价值在将来升级：
///   - 以后加新功能只需新增 0002_xxx.sql（加列/加表）
///   - 老数据库启动时：0001 已执行跳过 → 只执行 0002 → 表结构自动升级
///   - 不用删库、不用手动跑 SQL、不会丢数据
///
/// 我们稍后在 migrations/ 目录里放：
///   - 0001_init.sql：创建所有表
///
/// 好处：数据库结构用 SQL 文件管理，团队协作/部署升级都清晰。
async fn run_migrations(pool: &SqlitePool) -> Result<(), sqlx::Error>
{
    // migrate!() 是宏，括号里写相对 src/ 的路径
    sqlx::migrate!("./migrations")
        // 执行所有未执行的迁移
        .run(pool)
        .await?;

    tracing::info!("数据库迁移完成");
    Ok(())
}
