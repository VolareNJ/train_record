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
///
/// 步骤：
///   1. SqliteConnectOptions：配置如何连接（文件路径 + 创建缺失文件）
///   2. SqlitePoolOptions：配置连接池大小
///   3. .connect()：真正建立连接池
///   4. run_migrations()：确保表结构存在
pub async fn init_pool(config: &AppConfig) -> Result<SqlitePool, sqlx::Error>
{
    // 【教学：Builder 模式】
    // Rust 生态常用"链式调用"构造配置：每个方法返回自身，可连写
    let connect_options = SqliteConnectOptions::from_str(&config.database_path)?
        // create_if_missing(true)：文件不存在就自动创建
        .create_if_missing(true)
        // 启用外键约束（我们表之间有 FOREIGN KEY 关联）
        .foreign_keys(true)
        // 日志级别：连接失败等才打印，避免刷屏
        .log_statements(log::LevelFilter::Debug);

    // 连接池配置
    let pool = SqlitePoolOptions::new()
        // 最大连接数。SQLite 单文件，并发写需要排队，5 个够用
        .max_connections(5)
        // 连接空闲超时
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
