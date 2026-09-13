// ============================================================
// api/grpc/convert.rs —— 转换层：REST 读模型 ⇄ proto 消息（M9 第 5 步）
// ============================================================
// 【教学：这一层为什么必须存在？】
// gRPC 的输入输出是 **proto 消息**（pb::Phase），而业务代码产出的是
// **REST 的读模型 DTO**（PhaseOut）。两者形状很像但**不是同一个类型**：
//   · pb 类型是 protoc 生成的（字段编号驱动、PartialEq、用于线上传输）
//   · DTO 是手写的（serde 驱动、用于 JSON/内部传递）
// 直接让业务层返回 pb 类型也能跑，但会带来三个问题：
//   ① pb 类型里 `optional` 字段全是 Option、`repeated` 全是 Vec——
//      业务代码要到处处理"传输细节"，可读性差
//   ② REST 层一旦需要 pb 类型，就要把"protobuf 依赖"渗进 M8 的代码
//      （分层被打破：HTTP 出口不该知道 gRPC 的存在）
//   ③ 契约字段改名时，波及范围会扩散到业务层
// 所以：**边界处转换一次，内部保持自己的语言**。这是分层的常规代价与收益。
//
// 【教学：为什么用 From trait，而不是一堆 to_proto() 方法？】
//   · `pb::Phase::from(&out)` / `out.into()` 是 Rust 的惯用写法，读起来像类型转换
//   · 配合泛型代码能自动工作：`.iter().map(pb::Phase::from).collect()`
//     —— 一行把 Vec<PhaseOut> 变成 Vec<pb::Phase>（本项目大量使用迭代器适配器）
//   · 反向转换（请求）也对称：`PhaseCreateReq::from(&req)`
//
// 【教学：转换里能做什么、不能做什么？】
//   ✅ 字段改名（REST 的 "1rm" → proto 的 one_rm）
//   ✅ Option → Option（None 表示"不传"，proto 里就是字段缺省）
//   ✅ 实时计算派生值（1RM 用 calc::epley_1rm 现算）
//   ❌ 查数据库（转换是纯函数，async 不进来——要查库在 service 里查完再转）
//   ❌ 业务校验（校验属于 rest 层的查询函数，转换不做"判断"）
//
// ⚠️ 一个已知的"字段来源差异"（见 pb::Record 的 body_part 注释）：
//   REST 的 RecordOut（写路径返回值）**不含部位**，而 proto 的 Record 有 body_part。
//   M9 的选择：留空串 + 文档说明（最小改动），而不是改 M8 的响应形状。
//   教学点：两个出口的 DTO 不一致时，有三种应对——
//     ① 统一形状（改 REST，影响老客户端）② 留空/缺省 + 文档（本阶段选择）
//     ③ 转发时多查一次补全（多一次查询换一致性）
//   没有标准答案，取决于"这个字段对客户端有多重要"。需要部位请看
//   GetDayRecords / StreamRecords（它们的部位来自 JOIN，一定有值）。
// ============================================================

use super::pb;
use crate::{
    api::rest::{exercises, phases, plans, records, stats},
    calc::epley_1rm,
    models::User,
};

// ============================================================
// 一、认证：models::User → pb::User
// ============================================================
// 【教学：为什么从 models::User 转，而不是从 REST 的 UserOut 转？】
// gRPC 的 Login 走的是"自己查用户 + 复用 auth::create_session"，
// 没有经过 REST 的 login handler，所以源头是领域模型 User。
// ⚠️ 安全纪律：只挑安全字段（id/username/is_admin/body_weight），
//    **绝不**把 password_hash 带进 pb::User——它压根没这个字段（契约即防线）。
impl From<&User> for pb::User
{
    fn from(u: &User) -> Self
    {
        Self {
            id: u.id,
            username: u.username.clone(),
            is_admin: u.is_admin,
            body_weight: u.body_weight,
        }
    }
}

// ============================================================
// 二、阶段：PhaseOut → pb::Phase
// ============================================================
// 字段一一对应（含派生字段 days，REST 已经算好了）。
impl From<&phases::PhaseOut> for pb::Phase
{
    fn from(p: &phases::PhaseOut) -> Self
    {
        Self {
            id: p.id,
            name: p.name.clone(),
            note: p.note.clone(),
            start_date: p.start_date.clone(),
            archived: p.archived,
            days: p.days,
        }
    }
}

// ============================================================
// 三、动作：ExerciseOut → pb::Exercise（★ 挖空练习）
// ============================================================
// 【教学：这个转换的"教学价值"在哪？】
// 它是"派生字段"的典型：best_1rm / last_record_date 都是 REST 层
// 查库算出来的（见 rest/exercises.rs 的 exercise_out），gRPC 只是搬运。
// 注意两个 Option 字段：REST 用 None 表示"从未训练"，proto 里就是不传
// （optional 字段）——两边语义天然吻合，直接 clone 过去即可。
//
// 【实现步骤】
// 1. Self { id: ex.id, name: ex.name.clone(), ... } —— 逐字段照抄
// 2. 字符串字段要 .clone()（参数是 &ExerciseOut，不能拿走所有权）
// 3. best_1rm / last_record_date 直接赋 ex.xxx.clone()（Option 对 Option）
impl From<&exercises::ExerciseOut> for pb::Exercise
{
    // ⚠️ 挖空练习期间加 allow 消除 unused 警告，实现完成后可删
    #[allow(unused)]
    fn from(ex: &exercises::ExerciseOut) -> Self
    {
        // 【实现步骤】见上方注释
        todo!("M9 练习：ExerciseOut → pb::Exercise 转换实现") // 【待实现】
    }
}

// ============================================================
// 四、模板：TemplateOut → pb::Template
// ============================================================
impl From<&plans::TemplateItemOut> for pb::TemplateItem
{
    fn from(i: &plans::TemplateItemOut) -> Self
    {
        Self {
            id: i.id,
            exercise_id: i.exercise_id,
            exercise_name: i.exercise_name.clone(),
            plan_sets: i.plan_sets,
            plan_reps: i.plan_reps,
        }
    }
}

impl From<&plans::TemplateOut> for pb::Template
{
    fn from(t: &plans::TemplateOut) -> Self
    {
        Self {
            id: t.id,
            phase_id: t.phase_id,
            name: t.name.clone(),
            // 【教学：一次迭代器适配器完成"嵌套 Vec 转换"】
            // iter() → map(From) → collect()：Vec<TemplateItemOut> → Vec<pb::TemplateItem>
            items: t.items.iter().map(pb::TemplateItem::from).collect(),
        }
    }
}

// ============================================================
// 五、计划：PlanOut → pb::Plan
// ============================================================
impl From<&plans::PlanItemOut> for pb::PlanItem
{
    fn from(i: &plans::PlanItemOut) -> Self
    {
        Self {
            id: i.id,
            exercise_id: i.exercise_id,
            exercise_name: i.exercise_name.clone(),
            body_part: i.body_part.clone(),
            plan_sets: i.plan_sets,
            plan_reps: i.plan_reps,
            plan_weight: i.plan_weight,
            plan_rest: i.plan_rest,
            plan_key_points: i.plan_key_points.clone(),
            plan_note: i.plan_note.clone(),
        }
    }
}

impl From<&plans::PlanOut> for pb::Plan
{
    fn from(p: &plans::PlanOut) -> Self
    {
        Self {
            id: p.id,
            phase_id: p.phase_id,
            date: p.date.clone(),
            note: p.note.clone(),
            items: p.items.iter().map(pb::PlanItem::from).collect(),
        }
    }
}

// ============================================================
// 六、今日卡片：TodayOut → pb::TodayView（★ 挖空练习）
// ============================================================
// 【教学：这是最"难"的一个转换，难在**三层嵌套 + 两个 Option**】
//   TodayOut
//     ├── phase: Option<TodayPhaseOut>        → optional PhaseBrief
//     ├── date: String                        → string
//     └── plan:  Option<TodayPlanOut>         → optional TodayPlan
//           └── items: Vec<TodayItemOut>      → repeated TodayItem
//                 └── last_record: Option<..> → optional LastRecord
//
// 【实现步骤（建议先分解再组装，别一次写一大坨）】
// 1. 先给三个内层类型各写一个 From（它们各自都很短）：
//      impl From<&records::TodayPhaseOut>  for pb::PhaseBrief
//      impl From<&records::TodayPlanOut>   for pb::TodayPlan
//      impl From<&records::TodayItemOut>   for pb::TodayItem
//          （内层还要 LastRecordOut → pb::LastRecord，一起写了）
// 2. 最后 TodayView 只需要三行：
//      phase: v.phase.as_ref().map(pb::PhaseBrief::from),
//      date:  v.date.clone(),
//      plan:  v.plan.as_ref().map(pb::TodayPlan::from),
// 3. 关键点：**Option<&T> → Option<U> 用 as_ref() + map()**
//      .as_ref() 把 &Option<T> 变成 Option<&T>（借用，不复制整个结构体）
//      .map(From::from) 对 Some 里的引用做转换，None 原样传下去
//      —— 这是 Rust 最常见的"可选嵌套转换"写法，务必练熟。
// 4. 别忘 last_record 也是 Option，同理处理（提示：它在 TodayItemOut 里）。
impl From<&records::TodayPhaseOut> for pb::PhaseBrief
{
    fn from(p: &records::TodayPhaseOut) -> Self
    {
        Self {
            id: p.id,
            name: p.name.clone(),
            days: p.days,
        }
    }
}

impl From<&records::LastRecordOut> for pb::LastRecord
{
    fn from(r: &records::LastRecordOut) -> Self
    {
        Self {
            id: r.id,
            weight: r.weight,
            sets: r.sets,
            reps: r.reps,
            rest: r.rest,
            feeling: r.feeling.clone(),
            strategy: r.strategy.clone(),
        }
    }
}

impl From<&records::TodayItemOut> for pb::TodayItem
{
    fn from(i: &records::TodayItemOut) -> Self
    {
        Self {
            id: i.id,
            exercise_id: i.exercise_id,
            exercise_name: i.exercise_name.clone(),
            body_part: i.body_part.clone(),
            plan_sets: i.plan_sets,
            plan_reps: i.plan_reps,
            plan_weight: i.plan_weight,
            plan_rest: i.plan_rest,
            plan_key_points: i.plan_key_points.clone(),
            last_record: i.last_record.as_ref().map(pb::LastRecord::from),
        }
    }
}

impl From<&records::TodayPlanOut> for pb::TodayPlan
{
    fn from(p: &records::TodayPlanOut) -> Self
    {
        Self {
            id: p.id,
            note: p.note.clone(),
            items: p.items.iter().map(pb::TodayItem::from).collect(),
        }
    }
}

impl From<&records::TodayOut> for pb::TodayView
{
    // ⚠️ 挖空练习期间加 allow 消除 unused 警告，实现完成后可删
    #[allow(unused)]
    fn from(v: &records::TodayOut) -> Self
    {
        // 【实现步骤】见本节上方注释（三层嵌套 + 两个 Option）
        todo!("M9 练习：TodayOut → pb::TodayView 转换实现") // 【待实现】
    }
}

// ============================================================
// 七、记录：两种来源 → 同一个 pb::Record
// ============================================================
// 【教学：同一个 proto 消息，两个来源，一条注释说清差异】
//   来源 A：RecordOut（upsert/update 的返回值）—— 没有部位
//   来源 B：RecordRow（range/day 查询：记录 + 动作名 + 部位）—— 字段最全
// 转换里 body_part 因此不同（空串 vs 真实值），已在 proto 注释里写明。
impl From<&records::RecordOut> for pb::Record
{
    fn from(r: &records::RecordOut) -> Self
    {
        Self {
            id: r.id,
            exercise_id: r.exercise_id,
            exercise_name: r.exercise_name.clone(),
            // ⚠️ REST 的 RecordOut 不含部位 → 留空（见文件头"字段来源差异"）
            body_part: String::new(),
            record_date: r.record_date.clone(),
            mode: r.mode.clone(),
            weight: r.weight,
            sets: r.sets,
            reps: r.reps,
            rest: r.rest,
            feeling: r.feeling.clone(),
            strategy: r.strategy.clone(),
            key_points: r.key_points.clone(),
            // 【教学：派生值现算】1RM 不落库（存了就会与原始数据漂移），
            // 转发时用 calc.rs 的纯函数算一次，成本和读字段差不多。
            one_rm: epley_1rm(r.weight, r.reps),
            completed: r.completed,
        }
    }
}

// 【教学：为什么这条转换不是 `impl From`，而是一个函数？】（同文件末尾 series_point）
// 因为同一目标类型 pb::Record 已经有 From<&RecordOut> 了，再加一个
// From<&RecordRow> 也是合法的（不同源类型），但两个 impl 并存时
// **调用点必须靠类型推断选哪个**，可读性不如“函数带名字”直接：
//     pb::Record::from(&row)      ← 读代码的人要停下来看 row 是什么类型
//     convert::record_from_row(&row)   ← 一眼知道来源
// （两者都能用；本项目在“多来源转换”处统一用函数。）
pub(crate) fn record_from_row(row: &records::RecordRow) -> pb::Record
{
    let r = &row.record;
    pb::Record {
        id: r.id,
        exercise_id: r.exercise_id,
        exercise_name: row.exercise_name.clone(),
        body_part: row.body_part.clone(),
        record_date: r.record_date.clone(),
        mode: r.mode.clone(),
        weight: r.weight,
        sets: r.sets,
        reps: r.reps,
        rest: r.rest,
        feeling: r.feeling.clone(),
        strategy: r.strategy.clone(),
        key_points: r.key_points.clone(),
        one_rm: epley_1rm(r.weight, r.reps),
        completed: r.completed,
    }
}

// ============================================================
// 八、统计：日历 + 动作序列点
// ============================================================
impl From<&stats::CalendarOut> for pb::CalendarView
{
    fn from(c: &stats::CalendarOut) -> Self
    {
        Self {
            year: c.year.clone(),
            month: c.month.clone(),
            train_days: c.train_days.clone(),
        }
    }
}

/// 【教学：为什么不是 impl From？—— From 只能有一个参数】
/// 序列点需要两个入参（发起查询的 exercise_id + 记录本身），
/// 而 `From`/`Into` 是"一对一"的转换。这种情况用普通函数最清楚。
/// （替代方案：把 exercise_id 塞进一个元组再 From<(i64, &ExerciseRecordOut)>——
///   但元组可读性差，"专门的函数 + 好名字"更好。）
/// ⚠️ 挖空期间：只有待实现的 stream_exercise_series 会调它，
///    所以现在加 allow 消除 dead_code 警告；实现后删掉这行 allow。
#[allow(unused)]
pub(crate) fn series_point(
    exercise_id: i64,
    r: &stats::ExerciseRecordOut,
) -> pb::ExerciseSeriesPoint
{
    pb::ExerciseSeriesPoint {
        exercise_id,
        date: r.date.clone(),
        weight: r.weight,
        sets: r.sets,
        reps: r.reps,
        one_rm: r.one_rm,
    }
}

// ============================================================
// 九、反向转换：proto 请求 → REST 请求体
// ============================================================
// 【教学：为什么"反向"不能复用 REST 的 handler 参数？】
// gRPC 收到的是 proto 请求，但下游复用的是 REST 层的查询函数，
// 那些函数签名收的是 REST 的请求结构体（如 PhaseCreateReq）。
// 于是这里再做一次"协议 → 内部输入"的翻译。
// 好处：REST 层一行不改就同时服务两个出口；代价：多一层结构体。
// （这正是"两个出口共用一层内部输入类型"的设计——比各自写一套 SQL 便宜得多。）
impl From<&pb::CreatePhaseRequest> for phases::PhaseCreateReq
{
    fn from(req: &pb::CreatePhaseRequest) -> Self
    {
        Self {
            name: req.name.clone(),
            note: req.note.clone(),
            start_date: req.start_date.clone(),
        }
    }
}

impl From<&pb::UpdatePhaseRequest> for phases::PhaseUpdateReq
{
    fn from(req: &pb::UpdatePhaseRequest) -> Self
    {
        Self {
            name: req.name.clone(),
            note: req.note.clone(),
            start_date: req.start_date.clone(),
        }
    }
}

// 【教学：这里体现了"默认值该放哪"的老问题】
// REST 的 ExerciseCreateReq 用 serde 默认值兜底（不传 = "bar"/20.0/kg/3/8）。
// gRPC 的 optional 字段不传也是 None，于是**转换层要复用同一套默认值**——
// 直接调用 rest::exercises 里那几个 pub(crate) default_* 函数，
// 而不是在这里重写一遍字面量（重写 = 两个出口的默认值以后会漂移）。
impl From<&pb::CreateExerciseRequest> for exercises::ExerciseCreateReq
{
    fn from(req: &pb::CreateExerciseRequest) -> Self
    {
        Self {
            name: req.name.clone(),
            body_part: req.body_part.clone(),
            default_mode: req
                .default_mode
                .clone()
                .unwrap_or_else(exercises::default_mode),
            bar_weight: req.bar_weight.unwrap_or_else(exercises::default_bar_weight),
            default_unit: req
                .default_unit
                .clone()
                .unwrap_or_else(exercises::default_unit),
            default_sets: req.default_sets.unwrap_or_else(exercises::default_sets),
            default_reps: req.default_reps.unwrap_or_else(exercises::default_reps),
            key_points: req.key_points.clone().unwrap_or_default(),
        }
    }
}

impl From<&pb::UpdateExerciseRequest> for exercises::ExerciseUpdateReq
{
    fn from(req: &pb::UpdateExerciseRequest) -> Self
    {
        Self {
            name: req.name.clone(),
            body_part: req.body_part.clone(),
            default_mode: req.default_mode.clone(),
            bar_weight: req.bar_weight,
            default_unit: req.default_unit.clone(),
            default_sets: req.default_sets,
            default_reps: req.default_reps,
            key_points: req.key_points.clone(),
        }
    }
}

impl From<&pb::TemplateItemInput> for plans::TemplateItemReq
{
    fn from(item: &pb::TemplateItemInput) -> Self
    {
        Self {
            exercise_id: item.exercise_id,
        }
    }
}

impl From<&pb::CreateTemplateRequest> for plans::TemplateReq
{
    fn from(req: &pb::CreateTemplateRequest) -> Self
    {
        Self {
            name: req.name.clone(),
            items: req.items.iter().map(plans::TemplateItemReq::from).collect(),
        }
    }
}

impl From<&pb::UpdateTemplateRequest> for plans::TemplateReq
{
    fn from(req: &pb::UpdateTemplateRequest) -> Self
    {
        Self {
            name: req.name.clone(),
            items: req.items.iter().map(plans::TemplateItemReq::from).collect(),
        }
    }
}

impl From<&pb::PlanItemInput> for plans::PlanItemReq
{
    fn from(item: &pb::PlanItemInput) -> Self
    {
        Self {
            exercise_id: item.exercise_id,
            plan_sets: item.plan_sets,
            plan_reps: item.plan_reps,
            plan_weight: item.plan_weight,
            plan_rest: item.plan_rest,
            plan_key_points: item.plan_key_points.clone(),
            plan_note: item.plan_note.clone(),
        }
    }
}

impl From<&pb::CreatePlanRequest> for plans::PlanReq
{
    fn from(req: &pb::CreatePlanRequest) -> Self
    {
        Self {
            date: req.date.clone(),
            note: req.note.clone(),
            items: req.items.iter().map(plans::PlanItemReq::from).collect(),
        }
    }
}

impl From<&pb::UpdatePlanRequest> for plans::PlanReq
{
    fn from(req: &pb::UpdatePlanRequest) -> Self
    {
        Self {
            date: req.date.clone(),
            note: req.note.clone(),
            items: req.items.iter().map(plans::PlanItemReq::from).collect(),
        }
    }
}

impl From<&pb::UpsertRecordRequest> for records::RecordCreateReq
{
    fn from(req: &pb::UpsertRecordRequest) -> Self
    {
        Self {
            weight: req.weight,
            sets: req.sets,
            reps: req.reps,
            rest: req.rest,
            feeling: req.feeling.clone(),
            strategy: req.strategy.clone(),
            key_points: req.key_points.clone(),
            completed: req.completed,
        }
    }
}

impl From<&pb::UpdateRecordRequest> for records::RecordUpdateReq
{
    fn from(req: &pb::UpdateRecordRequest) -> Self
    {
        Self {
            weight: req.weight,
            sets: req.sets,
            reps: req.reps,
            rest: req.rest,
            feeling: req.feeling.clone(),
            strategy: req.strategy.clone(),
            key_points: req.key_points.clone(),
            completed: req.completed,
        }
    }
}
