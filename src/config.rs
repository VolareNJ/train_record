// ============================================================
// config.rs —— 应用配置模块
// ============================================================
// 【教学说明】
// 这个模块负责"从哪里读取配置"。为什么需要它？
// 因为我们的程序要部署在不同环境（本地电脑、服务器），
// 端口、数据库位置、会话密钥这些不能写死在代码里，
// 而是通过【环境变量】或【默认值】提供。
//
// 设计模式：Config::from_env() 是"构造函数"
//   - 读取 PORT（端口，默认 8080）
//   - 读取 DATABASE_PATH（数据库文件路径，默认 train_record.db）
//   - 读取 SESSION_SECRET（会话签名密钥，默认开发用值）
// 默认值让程序在本地无需任何配置就能跑起来；
// 部署到服务器时再通过环境变量覆盖。
// ============================================================

/// 应用配置结构体
///
/// 【教学说明】
/// struct 是 Rust 里组织相关数据的容器。
/// 我们把"程序运行需要的所有配置"打包成一个 AppConfig。
/// 之后 main.rs 里只需要 `let config = AppConfig::from_env();`
/// 就能一次性拿到所有配置，非常干净。
#[derive(Clone)]
pub struct AppConfig
{
    /// 服务器监听端口。默认 8080
    pub port: u16,
    /// SQLite 数据库文件路径。默认 "train_record.db"
    /// 这个文件会自动创建，所有数据都存在里面
    pub database_path: String,
    /// 会话签名密钥（用于保护登录 cookie）
    /// 生产环境必须通过环境变量 SESSION_SECRET 提供！
    pub session_secret: String,
}

impl AppConfig
{
    /// 从环境变量读取配置，缺失时使用默认值
    ///
    /// 【教学说明】
    /// - std::env::var() 读取环境变量，返回 Result
    /// - .unwrap_or_else(...) 是 Result 的方法：读不到就用默认值
    /// - 这种"环境变量 + 默认值"模式是 Rust 服务端项目的标准做法
    ///
    /// 为什么用 String::from 而不是字符串字面量？
    /// env::var 返回 String，为了类型一致这里也用 String。
    pub fn from_env() -> Self
    {
        // 端口：默认 8080。注意 parse() 把 String 转成 u16
        let port = std::env::var("PORT")
            .unwrap_or_else(|_| "8080".to_string())
            .parse()
            .expect("PORT 必须是数字");

        // 数据库路径：默认放在项目根目录的 train_record.db
        let database_path =
            std::env::var("DATABASE_PATH").unwrap_or_else(|_| "train_record.db".to_string());

        // 会话密钥：开发默认值。生产必须覆盖！
        // TODO(M1): 生产环境应从环境变量读取，否则有安全风险
        let session_secret = std::env::var("SESSION_SECRET")
            .unwrap_or_else(|_| "dev-only-secret-change-me".to_string());

        Self {
            port,
            database_path,
            session_secret,
        }
    }
}

// ============================================================
// 【测试教学】
// 这里是单元测试。cargo test 会执行 #[cfg(test)] 模块里的函数。
// 好处：验证 from_env() 在无环境变量时能正确返回默认值。
// 运行方式：cargo test
// ============================================================
#[cfg(test)]
mod tests
{
    use super::*;

    #[test]
    fn from_env_uses_defaults()
    {
        // 手动删除环境变量，确保测试用默认值
        // 【教学】unsafe 的 env::remove_var 在 Rust 2024 里需要 unsafe 块
        // 因为并发读写环境变量本身是不安全操作
        unsafe {
            std::env::remove_var("PORT");
            std::env::remove_var("DATABASE_PATH");
        }

        let config = AppConfig::from_env();
        // assert_eq! 是断言宏：如果两边不等就 panic，测试失败
        assert_eq!(config.port, 8080);
        assert_eq!(config.database_path, "train_record.db");
    }
}
