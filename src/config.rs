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
    ///
    /// 【教学：session_secret 是干什么的？】
    /// 背景：HTTP 是"无记忆"的，服务器记不住"你是谁"。
    /// M1 登录功能要让服务器记住登录状态，做法是：
    ///   1. 登录成功 → 服务器发一张"通行证"（cookie）
    ///   2. 之后每次请求浏览器自动带上它，服务器一看就知道"这是张三"
    ///
    /// 问题：通行证是浏览器里的一串文本，会被伪造。
    /// 如果通行证内容是 user_id=1，黑客可以自己构造一个冒充管理员。
    ///
    /// 【完整类比：门卫大叔 + 盖章通行证】
    /// 想象公司大楼门口的门卫大叔（服务器）：
    ///   第一次进门（登录）：报姓名 + 出示身份证（账号+密码），
    ///     大叔核实后发你一张盖了章的通行证（cookie + token）
    ///   之后每次进门（请求）：亮通行证即可，不用再报身份证
    ///   大叔验章：只有他的章能盖出这种印 → 通行证是真的（防伪造）
    ///   看内容：通行证写着"张三 007"（user_id）→ 识别身份
    ///   查登记簿：007 是管理员 → 放行进管理室（查 is_admin 权限）
    ///
    /// 三个"谁"：
    ///   1. 章是谁的？→ 服务器的！客户端只有通行证、没有章，所以伪造不了
    ///   2. 谁生成？  → 服务器管理员部署时配置（环境变量 SESSION_SECRET）
    ///   3. 谁保管？  → 服务器，绝不下发给浏览器
    ///
    /// session_secret 验证的是"通行证真伪"，不是"你是谁"：
    ///   验章 → 通行证是服务器发的真货；看内容 → 识别身份；查登记簿 → 判断权限
    /// 账号密码 = 第一次进门的身份证（只用一次）
    /// 通行证   = 之后每次亮的东西（cookie/session）
    /// is_admin = 登记簿里的角色记录（数据库字段）
    ///
    /// ⚠️ 项目实现说明：上面是经典的"签名验签"方案。
    /// 本项目 M1 实际用更简单的"查数据库"方案：
    ///   登录成功 → 生成随机 token（uuid）→ 存进 sessions 表
    ///   每次请求 → 拿 cookie 里的 token 去数据库查，查得到就是有效 session
    /// session_secret 目前只是预留字段，将来升级签名方案时才真正用到。
    ///
    /// 为什么叫 secret：必须保密，只有服务器自己知道。
    ///
    /// 阶段划分：
    ///   M0：这个字段只是存着，谁都没读它
    ///   M1：登录功能才会用它给 cookie 签名 / 验签（预留，M1 实际用数据库方案）
    /// 一句话：它是 M1 登录用的"私章"，现在只是提前占个位置。
    ///
    /// 默认值 "dev-only-secret-change-me" 是开发用临时章：
    ///   本地开发：随便一个值就能跑，方便
    ///   生产环境：默认值写在代码里，黑客知道就能伪造所有登录状态 → 必须覆盖！
    ///   所以部署时要用 SESSION_SECRET=一串随机长字符串 环境变量换掉它
    pub session_secret: String,
    /// 首次启动时创建的管理员用户名（环境变量 ADMIN_USERNAME）
    /// 为空 = 不自动创建（适合已有用户的部署）
    /// 【教学】首次部署需要"第一个管理员"，没有它就没人能创建用户。
    /// 两种方案：启动脚本创建 / 环境变量指定。本项目用环境变量。
    pub admin_username: String,
    /// 首次启动时创建的管理员密码（环境变量 ADMIN_PASSWORD）
    /// 生产环境必须设置！默认空 = 不自动创建管理员
    pub admin_password: String,
    /// 身体部位标准显示顺序（环境变量 BODY_PART_ORDER，逗号分隔）
    /// 用于 today / 模板编辑 / 计划编辑三处的组间排序。
    /// 默认 = 三分化习惯：腿 → 背 → 胸 → 核心 → 手臂 → 肩。
    /// 【M4 修订：用户要求"加一个修改静态排序的地方"】
    /// 方案：环境变量注入（本项目配置统一走"环境变量 + 默认值"模式），
    /// 无需数据库迁移、无需改代码 —— 改部署环境变量即可调整顺序。
    /// 注：部位名必须与 exercises.body_part 存的字面值一致（如"肩"非"肩部"）。
    pub body_part_order: Vec<String>,
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
        // 【函数式设计：依赖注入】
        // 我们不直接读环境变量，而是把"读取动作"作为函数参数传下去。
        // 好处：
        //   1. from_reader 是纯函数，只根据参数算出结果，没有副作用
        //   2. 测试时可以传入假读取器，不用动真实环境变量（因此无需 unsafe）
        //
        // 【教学：为什么用 .ok() 而不是 match 或 ?】
        //   std::env::var() 返回 Result<String, VarError>，
        //   但 from_reader 要求闭包返回 Option<String>。
        //   .ok() 把 Result 转成 Option：Ok→Some，Err→None（错误被"吞掉"）。
        //
        //   为什么不用 ?：
        //     1. 编译不过！? 只能解同类型：返回 Option 的闭包里
        //        不能对 Result 用 ?（类型不匹配，E0277）
        //     2. 语义不对！? 是"出错就中断"，而我们的需求是
        //        "没读到就当没值，继续用默认值"（优雅降级，不是快速失败）
        //
        //   为什么不用 match：可以但啰嗦（三行 vs 一行），
        //   .ok() 是标准库惯用适配器，意图更清晰。
        Self::from_reader(|name| std::env::var(name).ok())
    }

    /// 纯函数版构造函数：接受一个"按名字读配置值"的函数
    ///
    /// 【教学说明】
    /// - Fn(&str) -> Option<String> 是函数类型：传入名字，返回 Option 值
    /// - 闭包 |name| std::env::var(name).ok() 把 Result 转成 Option
    /// - .ok() 是 Result 的适配器：Ok(v) -> Some(v)，Err(_) -> None
    /// - 这样 from_reader 与"真实环境变量"解耦，测试无需 unsafe
    ///
    /// 【教学：为什么 from_env 和 from_reader 分开？】
    /// 因为 from_reader 不关心数据从哪来，只接收一个"读取函数"。
    /// 这带来两个好处：
    ///   1. 【可测试】测试时传假读取器，不碰真实环境变量，永远可复现
    ///   2. 【可扩展】将来加配置文件读取，只需新增 from_file()：
    ///        内部照样调 from_reader(|name| 从 config.toml 读)
    ///      完全不用改 from_reader 的代码（这就是依赖注入的价值）
    /// 一句话：from_env 管"从哪读"，from_reader 管"怎么算"。
    fn from_reader<F>(read: F) -> Self
    where
        F: Fn(&str) -> Option<String>,
    {
        // 端口：默认 8080。注意 parse() 把 String 转成 u16
        let port = read("PORT")
            .unwrap_or_else(|| "8080".to_string())
            .parse()
            .expect("PORT 必须是数字");

        // 数据库路径：默认放在项目根目录的 train_record.db
        let database_path = read("DATABASE_PATH").unwrap_or_else(|| "train_record.db".to_string());

        // 会话密钥：开发默认值。生产必须覆盖！
        // TODO(M1): 生产环境应从环境变量读取，否则有安全风险
        //
        // 【教学：unwrap_or_else 是什么？】
        // 拆开看：
        //   read("SESSION_SECRET")          返回 Option<String>（Some 值 或 None）
        //   .unwrap_or_else(|| "默认值")    是 None 就用闭包里的默认值，是 Some 就用值
        // 大白话："有值用值，没值用默认值顶上。"
        //
        // 为什么叫 unwrap_or_else 而不叫 unwrap_or？
        //   unwrap_or(值)      ：默认值每次都提前算好
        //   unwrap_or_else(闭包)：默认值需要时才算（懒执行）
        // 这里 .to_string() 创建字符串有成本，用 _else 只有 None 时才创建。
        //
        // 注意：下面三行是同一个模式重复三次——
        //   read(名字).unwrap_or_else(默认值)
        // 理解了这一个，PORT 和 DATABASE_PATH 那两行也全懂了。
        let session_secret =
            read("SESSION_SECRET").unwrap_or_else(|| "dev-only-secret-change-me".to_string());

        // 管理员账号：默认不自动创建（空 = 跳过）
        // 【教学】M1 新增的两个配置项，模式与上面完全一样：
        //   read(名字).unwrap_or_else(默认值)
        // 生产环境部署时用环境变量 ADMIN_USERNAME / ADMIN_PASSWORD 指定。
        let admin_username = read("ADMIN_USERNAME").unwrap_or_default();
        let admin_password = read("ADMIN_PASSWORD").unwrap_or_default();

        // 身体部位显示顺序：BODY_PART_ORDER 逗号分隔字符串 → Vec<String>
        // 默认三分化：腿 → 背 → 胸 → 核心 → 手臂 → 肩
        // （用 split + filter + map + collect 函数式适配器链，空段自动丢弃）
        let body_part_order = read("BODY_PART_ORDER")
            .unwrap_or_else(|| "腿,背,胸,核心,手臂,肩".to_string())
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();

        Self {
            port,
            database_path,
            session_secret,
            admin_username,
            admin_password,
            body_part_order,
        }
    }
}

// ============================================================
// 【测试教学】
// 这里是单元测试。cargo test 会执行 #[cfg(test)] 模块里的函数。
// 好处：验证 from_reader() 在不同输入下能正确返回配置。
// 运行方式：cargo test
//
// 【函数式测试技巧】
// 我们测试的是"纯函数" from_reader，而不是会碰真实环境变量的
// from_env。这样：
//   1. 无需修改环境变量（避免 unsafe 的 remove_var）
//   2. 测试结果与机器当前环境无关，永远可复现
//   3. 闭包 |_| None 表示"所有配置都缺失"，完美模拟空环境
// ============================================================
#[cfg(test)]
mod tests
{
    use super::*;

    #[test]
    fn from_reader_uses_defaults_when_empty()
    {
        // |_| None：任何名字都读不到 → 全部走默认值
        // 闭包里的 _ 表示"忽略参数"（这里我们不需要读到的名字）
        let config = AppConfig::from_reader(|_| None);
        // assert_eq! 是断言宏：如果两边不等就 panic，测试失败
        assert_eq!(config.port, 8080);
        assert_eq!(config.database_path, "train_record.db");
        assert_eq!(config.session_secret, "dev-only-secret-change-me");
        // 新字段：管理员默认不自动创建（空字符串）
        assert_eq!(config.admin_username, "");
        assert_eq!(config.admin_password, "");
        // 新字段：部位顺序默认三分化（腿→背→胸→核心→手臂→肩）
        assert_eq!(
            config.body_part_order,
            vec!["腿", "背", "胸", "核心", "手臂", "肩"]
        );
    }

    #[test]
    fn from_reader_honors_custom_values()
    {
        // 模拟一个"什么都能读到"的环境：根据名字返回不同值
        let config = AppConfig::from_reader(|name| match name
        {
            "PORT" => Some("9000".to_string()),
            "DATABASE_PATH" => Some("/tmp/test.db".to_string()),
            "SESSION_SECRET" => Some("my-secret".to_string()),
            "ADMIN_USERNAME" => Some("admin".to_string()),
            "ADMIN_PASSWORD" => Some("admin-pass".to_string()),
            "BODY_PART_ORDER" => Some("背,胸,腿".to_string()),
            _ => None,
        });
        assert_eq!(config.port, 9000);
        assert_eq!(config.database_path, "/tmp/test.db");
        assert_eq!(config.session_secret, "my-secret");
        assert_eq!(config.admin_username, "admin");
        assert_eq!(config.admin_password, "admin-pass");
        assert_eq!(config.body_part_order, vec!["背", "胸", "腿"]);
    }
}
