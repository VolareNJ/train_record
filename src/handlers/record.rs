// ============================================================
// handlers/record.rs —— 训练记录（Record）的 HTTP 处理器
// ============================================================
// 【教学说明】
// 这个文件处理"训练时记录实际完成情况"的 HTTP 请求，分三块：
//
// 一、今日页（核心页）
//   GET  /today                            → 今日训练页（today）
//
// 二、单动作记录/编辑页
//   GET  /plans/{id}/record/{item_id}      → 单动作记录表单（record_form）
//
// 三、保存记录
//   POST /plans/{id}/record/{item_id}/save → 保存（插入或更新）（record_save）
//
// 📌 阶段要求：M4 你来实现本文件所有函数。
//   实现完成后对照检查（完整实现备份在 docs/learning_path/M4_ref/）。
// ============================================================

// 【教学：本文件用到的导入】
// 和 M3 的 plan.rs 对比，多了 Json（其实这文件不用 Json，但保留注释说明）：
// 主要新增：无——Path/Form/State 都是老朋友。
// 关键：Record / Plan / PlanItem / Exercise 模型 + AuthUser 守卫。
use axum::{
    extract::{Form, Path, State},
    response::{Html, Redirect},
};
use serde::Deserialize;
use sqlx::SqlitePool;

use crate::{
    AppState,
    error::AppError,
    handlers::auth::AuthUser,
    models::{Exercise, Phase, Plan, PlanItem, Record},
};

// ============================================================
// 【教学：从"计划"到"记录"的跨越 —— 本阶段核心】
// ============================================================
// M3 管的是"训练前"：把动作排成计划。
// M4 管的是"训练中/后"：把实际完成记下来。
//
// 三个核心认知：
//   1. 【一条记录 = 一次训练的动作汇总】
//      不是每组一条！plan_items 一行 = "卧推 4×8"，
//      records 一行 = "今天卧推实际做了 60kg × 4组 × 8次"。
//      组数 sets 只是记录里的一个数字。
//   2. 【记录挂计划项，也挂阶段】双挂靠：
//      plan_item_id → 这条记录属于哪个计划里的哪个动作
//      phase_id     → 这条记录属于哪个阶段（M5 历史按阶段筛选）
//   3. 【Upsert 语义】同一天同一计划项只应有一条记录：
//      有 → UPDATE（改旧值）；没有 → INSERT（新增）
//      绝不能每次都 INSERT（否则历史表出现"同一天同动作"多条记录）
//
// 这三个认知贯穿本文件所有函数，先记住它们。

// ============================================================
// 【教学：日期怎么来？—— 永远用 SQLite，不用 Rust 端】
// ============================================================
// 项目里所有"今天"都统一用：
//   SELECT date('now', 'localtime')
// 为什么不用 Rust 的 chrono/SystemTime？
//   1. 时区：数据库存的是 SQLite 的 localtime（中国时区），
//      Rust 端 SystemTime 是 UTC，两边对不上会差 8 小时
//   2. 一致性：计划创建、记录落库、坚持天数全用同一来源，
//      不会出现"计划是今天、记录是昨天"的边界 bug
// 记住：本项目凡是"今天/日期差"，都让 SQLite 算。

// ============================================================
// 第一部分：今日页（GET /today）
// ============================================================
/// 今日训练页：阶段 + 坚持天数 + 今天的计划动作清单 + 每个动作的状态
///
/// 【教学：今日页是"训练时的操作台"】
/// 用户训练时打开这个页面，一眼看到：
///   - 顶部：阶段名 + 已坚持 N 天 + 今天日期
///   - 中间：今天的计划动作清单（动作名 + 计划值）
///   - 每个动作：状态徽标（✅已训练 / ⬜未训练）+ 上次策略提示
///   - 点动作 → 进入记录/编辑页
///
/// 实现步骤：
/// 1. 签名：State + AuthUser
/// 2. 查进行中阶段：
///    SELECT * FROM phases WHERE user_id = ? AND archived = 0
///    ORDER BY created_at DESC LIMIT 1
///    → 没有 → 空态提示"暂无进行中阶段，请先创建"
/// 3. 查今天：SELECT date('now', 'localtime')
/// 4. 查今天的计划：
///    SELECT * FROM plans WHERE phase_id = ? AND date = ?
///    → 没有 → 空态提示"今天还没有计划"
/// 5. 查计划项（JOIN 动作名）：
///    SELECT pi.*, ex.name AS exercise_name
///    FROM plan_items pi
///    JOIN exercises ex ON pi.exercise_id = ex.id
///    WHERE pi.plan_id = ? ORDER BY pi.sort_order
/// 6. 每个计划项查"最近一条记录"判断状态 + 上次策略：
///    SELECT * FROM records WHERE plan_item_id = ?
///    ORDER BY record_date DESC, id DESC LIMIT 1
///    → 有记录 → ✅已训练 + 显示该条 strategy
///    → 无记录 → ⬜未训练
/// 7. 拼 HTML：阶段信息 + 计划动作列表（每行：动作名/计划值/状态/策略/记录链接）
pub async fn today(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
) -> Result<Html<String>, AppError>
{
    // TODO(M4): 学生实现（步骤见上方注释）
    unimplemented!("M4 学生实现：今日页")
}

// ============================================================
// 第二部分：单动作记录/编辑页（GET /plans/{id}/record/{item_id}）
// ============================================================
/// 显示某个计划项的记录/编辑表单
///
/// 【教学：两级路径参数 —— {id} 是计划，{item_id} 是计划项】
/// 路由 /plans/{id}/record/{item_id} 有两个参数：
///   {id}      → 计划 id（Path 第一个）
///   {item_id} → 计划项 id（Path 第二个）
/// axum 用元组提取：Path((id, item_id)): Path<(i64, i64)>
///
/// 页面分上下两区：
///   上半区：计划值（该动作计划做几组几次多重）+ 上次记录参考
///     （上次实际重量/组数/次数/感受/策略——渐进超负荷的"参照物"）
///   下半区：录入表单——实际重量（含换算器）、组数、次数、休息、
///     感受、策略、要领（预填动作库 key_points）
///
/// 【教学：为什么要显示"上次记录参考"？】
/// 渐进超负荷的核心动作是"这次比上次重/多"。
/// 没有上次数据，用户凭记忆加重量，容易加过头或没进步。
/// 参考 = 上次的实际记录（不是计划值！），让用户对比着填。
///
/// 实现步骤：
/// 1. 签名：State + AuthUser + Path((id, item_id))
/// 2. 验证计划归属：JOIN phases 查 user_id
/// 3. 验证计划项属于该计划：WHERE id = ? AND plan_id = ?（双条件防越权）
/// 4. 查动作信息（拿 key_points 预填 + bar_weight 给换算器）
/// 5. 查该计划项最近一条记录（有 → 编辑模式预填；无 → 空表单）
/// 6. 拼 HTML：计划值 + 上次参考 + 表单（含换算器挂载点）
pub async fn record_form(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path((id, item_id)): Path<(i64, i64)>,
) -> Result<Html<String>, AppError>
{
    // TODO(M4): 学生实现（步骤见上方注释）
    unimplemented!("M4 学生实现：单动作记录/编辑页")
}

// ============================================================
// 第三部分：保存记录（POST /plans/{id}/record/{item_id}/save）
// ============================================================
/// 处理记录表单提交：有记录 → UPDATE，无记录 → INSERT（Upsert）
///
/// 【教学：表单字段全用 String（M2 约定）】
/// 用户可能留空提交（""），如果字段声明成 f64/i64，
/// axum 反序列化 "" → f64 失败 → 直接 400 错误（体验差）。
/// 所以表单层全用 String，入库前 parse（与 exercises.rs 的 ExerciseForm 同款）。
///
/// 【教学：Upsert 的两种写法】
/// 方案 A（本项目用）：先查有没有 → 有则 UPDATE，无则 INSERT
///   优点：直白、逻辑清晰、教学友好
/// 方案 B：SQLite 的 INSERT ... ON CONFLICT DO UPDATE
///   优点：一条 SQL 搞定（M5/M7 打磨时再优化）
///
/// 【教学：校验规则 —— 负数拒绝】
/// weight/sets/reps/rest 必须 >= 0（训练数据不可能是负数）。
/// parse 成功但为负数 → Err(AppError::Validation("重量不能为负数"))
/// parse 失败（"abc"）→ 也要转成 Validation（不是 500！）
///
/// 实现步骤：
/// 1. 签名：State + AuthUser + Path((id, item_id)) + Form(form)
/// 2. 验证归属（同 record_form）
/// 3. parse 数字字段：weight → f64，sets/reps/rest → i64
///    （parse 失败 → Validation；负数 → Validation）
/// 4. 查该计划项最近一条记录（决定 INSERT 还是 UPDATE）
/// 5. 有记录 → UPDATE：
///    UPDATE records SET weight=?, sets=?, reps=?, rest=?,
///      feeling=?, strategy=?, key_points=?, mode=?
///    WHERE id = ?（按查到的记录 id）
/// 6. 无记录 → INSERT：
///    INSERT INTO records
///      (plan_item_id, phase_id, exercise_id, record_date,
///       weight, sets, reps, rest, feeling, strategy, key_points, mode)
///    VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
///    （phase_id/exercise_id 从计划项 JOIN 取；record_date = 今天）
/// 7. 重定向回 /today（今日页刷新后显示 ✅ 已训练）
pub async fn record_save(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path((id, item_id)): Path<(i64, i64)>,
    Form(form): Form<RecordForm>,
) -> Result<Redirect, AppError>
{
    // TODO(M4): 学生实现（步骤见上方注释）
    unimplemented!("M4 学生实现：保存记录")
}

// ============================================================
// 【教学：表单结构体 —— M4 的 RecordForm】
// ============================================================
/// 记录表单（字段全 String，入库前 parse）
///
/// 【教学：为什么 weight 不用 f64 而用 String？】
/// 同 M2 的 ExerciseForm：用户留空提交 "" 时，
/// f64 直接 400，String 能收到 "" 再判断。
/// 这里 weight 是"必填"（实际重量必须有），
/// 但 sets/reps/rest 也可能被用户清空——全用 String 统一处理。
#[derive(Debug, Deserialize)]
pub struct RecordForm
{
    /// 实际总重 kg（表单层 String，入库前 parse）
    pub weight: String,
    /// 实际组数
    pub sets: String,
    /// 实际次数
    pub reps: String,
    /// 组间休息秒（可空 → ""）
    pub rest: String,
    /// 感受（自由文本）
    pub feeling: String,
    /// 策略/后续安排
    pub strategy: String,
    /// 当次要领（预填动作库，可改）
    pub key_points: String,
    /// 录入时模式（bar/support/std/lb2kg）
    pub mode: String,
}

// ============================================================
// 【教学：解析表单数字的辅助函数 —— 空串 → 默认值】
// ============================================================
/// 把表单的字符串数字解析成 i64，空串/解析失败 → 返回默认值
///
/// 【教学：为什么要有这个辅助函数？】
/// 表单里 sets/reps/rest 用户可能留空，也可能是脏数据（"abc"）。
/// 如果每个字段都写一遍 match，代码重复 3 遍。
/// 抽成泛型函数：parse_or(字符串, 默认值) → 数字
/// （这里是教学版，只做 i64；M4 学生可按需扩展 f64 版）
///
/// 【教学：泛型 + FromStr 的写法】
/// fn parse_or<T: FromStr>(s: &str, default: T) -> T {
///     s.trim().parse::<T>().unwrap_or(default)
/// }
/// T 只要是"能从字符串解析的类型"（i64/f64 都实现了 FromStr）就能用。
/// unwrap_or(default)：解析成功用解析值，失败用默认值（不 panic）。
fn parse_or<T>(s: &str, default: T) -> T
where
    T: std::str::FromStr,
{
    s.trim().parse::<T>().unwrap_or(default)
}
