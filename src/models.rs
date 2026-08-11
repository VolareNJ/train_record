// ============================================================
// models.rs —— 领域模型模块
// ============================================================
// 【教学说明】
// 领域模型 = 业务概念在代码里的"数据结构"。
// 本项目有 7 个核心概念（对应数据库 7 张表）：
//   User(用户) Phase(阶段) Exercise(动作) Template(模板)
//   TemplateItem(模板项) Plan(计划) PlanItem(计划项) Record(训练记录)
//
// M0 阶段我们只声明结构体骨架，字段与数据库表一一对应。
// 实际增删改查(M1之后)再逐步填充方法。
// 先定义结构，让项目能编译、让大家理解模型长什么样。
// ============================================================

// 【教学：sqlx::FromRow】
// 这个 derive 让结构体可以直接从数据库查询结果转换：
//   let user: User = sqlx::query_as("SELECT ...").fetch_one(&pool).await?;
// sqlx 会自动按字段名匹配列。
use sqlx::FromRow;

// ============================================================
// 用户表：对应 users
// ============================================================
/// 用户
/// 【教学】每个字段注释对应数据库列的含义
#[derive(Debug, Clone, FromRow)]
pub struct User
{
    pub id: i64,
    /// 登录名（唯一）
    pub username: String,
    /// argon2 密码哈希（不存明文密码！）
    pub password_hash: String,
    /// 显示名称
    pub display_name: String,
    /// 是否管理员：0=否 1=是
    pub is_admin: bool,
    pub created_at: String,
}

// ============================================================
// 训练阶段：对应 phases
// 这是项目的核心概念！阶段 = 一段连续的训练时期。
// ============================================================
/// 训练阶段
#[derive(Debug, Clone, FromRow)]
pub struct Phase
{
    pub id: i64,
    pub user_id: i64,
    /// 阶段名，如 "phase1"
    pub name: String,
    /// 备注
    pub note: String,
    /// 开始日期（'YYYY-MM-DD'），用于计算"已坚持 N 天"
    pub start_date: Option<String>,
    /// 是否归档：0=进行中 1=已归档(只读)
    pub archived: bool,
    pub created_at: String,
}

// ============================================================
// 动作库：对应 exercises
// ============================================================
/// 动作（动作库中的一项）
#[derive(Debug, Clone, FromRow)]
pub struct Exercise
{
    pub id: i64,
    pub user_id: i64,
    /// 动作名，如 "深蹲"
    pub name: String,
    /// 部位分组：胸/背/腿/肩/臂/核心
    pub body_part: String,
    /// 默认模式：bar/support/std（M4 修订：lb2kg 移除，老数据归一到 std）
    pub default_mode: String,
    /// 默认杆重（bar 模式用）
    pub bar_weight: f64,
    /// 默认组数（建计划时预填）
    pub default_sets: i64,
    /// 默认次数（建计划时预填）
    pub default_reps: i64,
    /// 动作要领文本
    pub key_points: String,
    /// 部位内排序（同一 body_part 内从 1 开始；M4 修订新增）
    pub sort_order: i64,
    pub created_at: String,
}

// ============================================================
// 模板：对应 templates
// 模板 = 一组有序动作，如"推日模板"包含卧推、推举、臂屈伸
// ============================================================
/// 训练模板（绑定阶段）
#[derive(Debug, Clone, FromRow)]
pub struct Template
{
    pub id: i64,
    pub phase_id: i64,
    /// 模板名，如 "A分化"
    pub name: String,
    pub sort_order: i64,
}

/// 模板项（模板里的一个动作）
#[derive(Debug, Clone, FromRow)]
pub struct TemplateItem
{
    pub id: i64,
    pub template_id: i64,
    pub exercise_id: i64,
    pub sort_order: i64,
    /// 计划组数（空=用动作默认）
    pub plan_sets: Option<i64>,
    /// 计划次数
    pub plan_reps: Option<i64>,
}

// ============================================================
// 当日计划：对应 plans
// ============================================================
/// 当日计划（一次训练日）
#[derive(Debug, Clone, FromRow)]
pub struct Plan
{
    pub id: i64,
    pub phase_id: i64,
    /// 日期 'YYYY-MM-DD'（中国时区自然日）
    pub date: String,
    pub note: String,
    pub created_at: String,
}

/// 计划项（计划里的一个动作）
#[derive(Debug, Clone, FromRow)]
pub struct PlanItem
{
    pub id: i64,
    pub plan_id: i64,
    pub exercise_id: i64,
    pub sort_order: i64,
    /// 计划组数
    pub plan_sets: Option<i64>,
    /// 计划次数
    pub plan_reps: Option<i64>,
    /// 计划重量（总重 kg，可空）
    pub plan_weight: Option<f64>,
    /// 计划计重方式（bar/support/std；空 = 未预设，record_form 落回动作默认）
    /// 历史值 lb2kg（标准lb）已并入 std：lb 单位移到观测强度旁选择（M4 修订）
    pub plan_mode: Option<String>,
    /// 计划杆重规格（20/11.3/10/0；空 = 用动作默认）
    pub plan_bar_weight: Option<f64>,
    /// 计划休息秒（空 = 未预设）
    pub plan_rest: Option<i64>,
    /// 计划要领（空 = record_form 落回动作库 key_points）
    pub plan_key_points: Option<String>,
    /// 动作级备注（空 = 无备注；区别于 plans.note 整计划备注）
    pub plan_note: Option<String>,
}

// ============================================================
// ============================================================
// 【M4 修订：身体部位标准顺序 + 分组排序工具】
// 三个页面（today / templates/{id}/edit / plans/{id}）都要按 body_part
// 分组显示，且组间顺序要一致。
//
// 方案评估（用户问题 4 + 后续"加一个修改静态排序的地方"）：
//   ① 后端顺序注入（✅ 本方案）—— 顺序来自 AppConfig.body_part_order
//      （环境变量 BODY_PART_ORDER 逗号分隔，默认三分化：
//      腿→背→胸→核心→手臂→肩），不迁移、三处共用同一函数、
//      改部署环境变量即可调序（无需改代码）。
//   ② 加 body_part_sort_order 字段 —— 需迁移 0006 + 设置 UI，
//      灵活但重，当前收益低（部位集合固定）。
//   ③ 前端 JS 排序 —— 维护差、刷新闪跳，且后端渲染的 today 页无法复用。
//   结论：环境变量注入最符合本项目"环境变量 + 默认值"的配置模式。
//   注意：部位名必须与 exercises.body_part 的字面值一致（如"肩"非"肩部"）。
// ============================================================

/// 部位在标准顺序里的排序键；未收录的部位给一个很大的值（排最后，稳定兜底）
/// 【M4 修订：顺序由调用方传入（来自 AppConfig.body_part_order，环境变量可配）】
fn part_rank(part: &str, order: &[String]) -> usize
{
    order.iter().position(|p| p == part).unwrap_or(usize::MAX)
}

/// 把"动作行列表"按身体部位分组，且组间按标准顺序排序。
///
/// 输入：已按 sort_order 排好的 (部位, 行HTML) 迭代器（保序分组，组内即 sort_order 序）。
///       order：部位标准顺序（来自 AppConfig.body_part_order，环境变量可配）。
/// 输出：按 part_rank 升序排列的 (部位, 行列表) 向量 —— 组内顺序不变，
///       组间顺序由 order 决定。
/// 同 rank 的组（例如两个自定义部位都兜底）保持输入相对顺序（稳定排序）。
pub fn group_by_body_part<'a>(
    rows: impl Iterator<Item = (String, String)>,
    order: &[String],
) -> Vec<(String, Vec<String>)>
{
    // 保序分组（与 record.rs today 的 groups 逻辑一致）：
    //   找到同部位组 → 追加；找不到 → 开新组
    let mut groups: Vec<(String, Vec<String>)> = Vec::new();
    for (part, row) in rows
    {
        match groups.iter_mut().find(|(p, _)| *p == part)
        {
            Some((_, rows)) => rows.push(row),
            None => groups.push((part, vec![row])),
        }
    }
    // 稳定排序：相同 rank（如都兜底）保持输入顺序
    let mut keyed: Vec<(usize, (String, Vec<String>))> = groups
        .into_iter()
        .map(|g| (part_rank(&g.0, order), g))
        .collect();
    keyed.sort_by_key(|(rank, _)| *rank);
    keyed.into_iter().map(|(_, g)| g).collect()
}

// ============================================================
// 训练记录：对应 records
// ============================================================
/// 训练记录（动作级，一条 = 一次完成记录）
#[derive(Debug, Clone, FromRow)]
pub struct Record
{
    pub id: i64,
    /// 关联的计划项（若从计划录入）
    pub plan_item_id: Option<i64>,
    pub phase_id: i64,
    pub exercise_id: i64,
    /// 记录日期 'YYYY-MM-DD'
    pub record_date: String,
    /// 实际总重量 kg
    pub weight: f64,
    pub sets: i64,
    pub reps: i64,
    /// 组间休息秒
    pub rest: i64,
    /// 感受（自由文本）
    pub feeling: String,
    /// 策略/后续安排
    pub strategy: String,
    /// 当次要领
    pub key_points: String,
    /// 录入时模式
    pub mode: String,
    /// 是否已完成（0=未完成 1=已完成；M4 修订新增）
    /// 今日页只有 completed = 1 才标"✅已完成"
    pub completed: bool,
    pub created_at: String,
}
