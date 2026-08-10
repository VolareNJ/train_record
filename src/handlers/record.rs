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
// 主要新增：HashMap —— 建"动作 id → 动作名"索引（M3 同款模式，见 today 注释第 5 步）
// 关键：Record / Plan / PlanItem / Exercise 模型 + AuthUser 守卫。
use std::collections::HashMap;

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
/// 5. 查计划项（不带动作名，避免 JOIN 破坏 query_as）：
///    SELECT * FROM plan_items WHERE plan_id = ? ORDER BY sort_order
///    再查全部动作 → 建 HashMap<i64, String>（id → 名字）索引：
///    SELECT * FROM exercises WHERE user_id = ?
///    （M3 同款模式：查两次 + 内存索引。为什么不用 JOIN？
///     query_as 按列名匹配结构体，JOIN 多出的 exercise_name 列
///     与 PlanItem 不匹配，无法反序列化）
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
    // 1. 签名：State + AuthUser
    // 2. 查进行中阶段：
    //    SELECT * FROM phases WHERE user_id = ? AND archived = 0
    //    ORDER BY created_at DESC LIMIT 1
    //    → 没有 → 空态提示"暂无进行中阶段，请先创建"
    let current_phase = sqlx::query_as::<_, Phase>(
        "SELECT * FROM phases WHERE user_id = ? AND archived = 0 ORDER BY created_at DESC LIMIT 1",
    )
    .bind(&user.id)
    .fetch_optional(&state.pool)
    .await
    .map_err(AppError::Database)?
    .ok_or_else(|| AppError::NotFound("No phase running on your profile".to_string()))?;
    // 3. 查今天：SELECT date('now', 'localtime')
    let today_dt = sqlx::query_scalar::<_, String>("SELECT date('now', 'localtime')")
        .fetch_one(&state.pool)
        .await
        .map_err(AppError::Database)?;
    // 4. 查今天的计划：
    //    SELECT * FROM plans WHERE phase_id = ? AND date = ?
    //    → 没有 → 空态提示"今天还没有计划"
    let today_plan =
        sqlx::query_as::<_, Plan>("SELECT * FROM plans WHERE phase_id = ? AND date = ?")
            .bind(&current_phase.id)
            .bind(&today_dt)
            .fetch_optional(&state.pool)
            .await
            .map_err(AppError::Database)?
            .ok_or_else(|| AppError::NotFound("No plan set for today".to_string()))?;
    // 5. 查计划项（不带动作名，避免 JOIN 破坏 query_as）：
    //    SELECT * FROM plan_items WHERE plan_id = ? ORDER BY sort_order
    //    再查全部动作 → 建 HashMap<i64, String>（id → 名字）索引：
    //    SELECT * FROM exercises WHERE user_id = ?
    //    （M3 同款模式：查两次 + 内存索引。为什么不用 JOIN？
    //     query_as 按列名匹配结构体，JOIN 多出的 exercise_name 列
    //     与 PlanItem 不匹配，无法反序列化）
    let today_plan_items = sqlx::query_as::<_, PlanItem>(
        "SELECT * FROM plan_items WHERE plan_id = ? ORDER BY sort_order ASC",
    )
    .bind(&today_plan.id)
    .fetch_all(&state.pool)
    .await
    .map_err(AppError::Database)?;
    let id_to_name = sqlx::query_as::<_, Exercise>("SELECT * FROM exercises WHERE user_id = ?")
        .bind(&user.id)
        .fetch_all(&state.pool)
        .await
        .map_err(AppError::Database)?
        .iter()
        .map(|e| (e.id, e.name.clone()))
        .collect::<HashMap<i64, String>>();
    // 6. 每个计划项查"最近一条记录"判断状态 + 上次策略：
    //    SELECT * FROM records WHERE plan_item_id = ?
    //    ORDER BY record_date DESC, id DESC LIMIT 1
    //    → 有记录 → ✅已训练 + 显示该条 strategy
    //    → 无记录 → ⬜未训练
    let mut items_with_records = Vec::new();
    for item in &today_plan_items
    {
        let last = sqlx::query_as::<_, Record>(
            "SELECT * FROM records WHERE plan_item_id = ?
         ORDER BY record_date DESC, id DESC LIMIT 1",
        )
        .bind(item.id)
        .fetch_optional(&state.pool)
        .await
        .map_err(AppError::Database)?;
        items_with_records.push((item, last));
    }

    // 7. 拼 HTML：阶段信息 + 计划动作列表（每行：动作名/计划值/状态/策略/记录链接）

    // 7a. 坚持天数（start_date 为空 → 显示"未设置开始日期"）
    //     julianday 相减 = 自然日差（今天 8/10，开始 8/1 → 9 天）
    let persist_days = match &current_phase.start_date
    {
        Some(start_date) => sqlx::query_scalar::<_, i64>(
            "SELECT CAST(julianday('now','localtime') - julianday(?) AS INTEGER)",
        )
        .bind(start_date)
        .fetch_one(&state.pool)
        .await
        .map_err(AppError::Database)?
        .to_string(),
        None => "未设置开始日期".to_string(),
    };

    // 7b. 动作列表行（items_with_records = (计划项, 最近记录) 配对）
    let item_rows = items_with_records
        .iter()
        .map(|(item, last)| {
            // 动作名：从 HashMap 索引取（查不到显示 "?"，理论不发生）
            let ex_name = id_to_name
                .get(&item.exercise_id)
                .cloned()
                .unwrap_or_else(|| "?".to_string());
            // 计划值：组×次，重量有就带括号（None → "-"）
            let plan_value = format!(
                "{}组 * {}次{}",
                item.plan_sets.map_or("-".to_string(), |v| v.to_string()),
                item.plan_reps.map_or("-".to_string(), |v| v.to_string()),
                item.plan_weight
                    .map_or(String::new(), |v| format!("({v}kg)")),
            );
            // 状态徽标 + 上次策略提示
            let (badge, strategy_hint) = match last
            {
                Some(rec) => (
                    "✅已训练".to_string(),
                    format!("上次策略：{}", rec.strategy),
                ),
                None => ("⬜未训练".to_string(), String::new()),
            };
            format!(
                "<tr><td>{ex_name}</td><td>{plan_value}</td><td>{badge}</td>\
                 <td>{strategy_hint}</td>\
                 <td><a href=\"/plans/{plan_id}/record/{item_id}\">记录/编辑</a></td></tr>",
                ex_name = ex_name,
                plan_value = plan_value,
                badge = badge,
                strategy_hint = strategy_hint,
                plan_id = today_plan.id,
                item_id = item.id,
            )
        })
        .collect::<Vec<String>>()
        .join("\n");

    // 7c. 拼整页（风格与 M3 一致：h2 + 表格 + 返回链接）
    Ok(Html(format!(
        r#"
        <head><meta name="viewport" content="width=device-width, initial-scale=1.0"></head>
        <h2>今日训练({today_dt})</h2>
        <p>阶段：{phase_name} | 已坚持 {persist_days} 天</p>
        <table border="1"><tr><th>动作</th><th>计划值</th><th>状态</th><th>上次策略</th><th>操作</th></tr>
            {item_rows}
        </table>
        <p><a href="/">返回首页</a></p>
        "#,
        today_dt = today_dt,
        phase_name = current_phase.name,
        persist_days = persist_days,
        item_rows = item_rows,
    )))
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
///
/// 【教学：换算器挂载点（配合 static/weight_converter.js）】
/// record_form 页面要引入换算器脚本 + 提供挂载元素：
///   <script src="/static/weight_converter.js"></script>
///   <select id="mode-select">（bar/support/std/lb2kg，默认动作的 default_mode）
///   <input id="plate-input">     片重/支撑量
///   <input id="bar-input">       杆重（bar 模式才显示）
///   <input id="body-input">     体重（support 模式才显示）
///   <span id="result">           换算结果
///   <button id="fill-btn">        填入重量
///   页面 <body data-bar-weight="动作.bar_weight"> 提供初始杆重
/// 四种模式公式（与 JS 一致）：
///   bar     = 杆重 + 2×片重
///   support = 体重 − 支撑量（支撑器械标的是"抵消多少体重"，
///             如 90kg 体重 + 30kg 支撑做引体 → 实际负重 60kg）
///   std     = 片重
///   lb2kg   = 片重 × 0.4536
pub async fn record_form(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path((plan_id, item_id)): Path<(i64, i64)>,
) -> Result<Html<String>, AppError>
{
    // 1. 签名：State + AuthUser + Path((plan_id, item_id))
    // 2. 验证计划归属：JOIN phases 查 user_id
    let current_plan = sqlx::query_as::<_, Plan>(
        "SELECT p.* FROM plans p
        INNER JOIN phases ph ON p.phase_id = ph.id
        WHERE p.id = ? AND ph.user_id = ?",
    )
    .bind(&plan_id)
    .bind(&user.id)
    .fetch_optional(&state.pool)
    .await
    .map_err(AppError::Database)?
    .ok_or_else(|| AppError::NotFound("No plan found in such user and phase".to_string()))?;

    let phase = sqlx::query_as::<_, Phase>("SELECT * FROM phases WHERE id = ? AND user_id = ?")
        .bind(&current_plan.phase_id)
        .bind(&user.id)
        .fetch_optional(&state.pool)
        .await
        .map_err(AppError::Database)?
        .ok_or_else(|| AppError::NotFound("No phase found".to_string()))?;
    if phase.archived
    {
        return Err(AppError::Forbidden(
            "Can not edit archived phase".to_string(),
        ));
    }

    // 3. 验证计划项属于该计划：WHERE id = ? AND plan_id = ?（双条件防越权）
    // 3. 验证计划项属于该计划：双条件
    let plan_item =
        sqlx::query_as::<_, PlanItem>("SELECT * FROM plan_items WHERE id = ? AND plan_id = ?")
            .bind(&item_id)
            .bind(&current_plan.id)
            .fetch_optional(&state.pool)
            .await
            .map_err(AppError::Database)?
            .ok_or_else(|| AppError::NotFound("No plan item found".to_string()))?;

    // 4. 查动作信息（拿 key_points 预填 + bar_weight 给换算器）
    let exercise_details =
        sqlx::query_as::<_, Exercise>("SELECT * FROM exercises WHERE id = ? AND user_id = ?")
            .bind(&plan_item.exercise_id)
            .bind(&user.id)
            .fetch_optional(&state.pool)
            .await
            .map_err(AppError::Database)?
            .ok_or_else(|| AppError::NotFound("No such exercise".to_string()))?;
    // 5. 查该计划项最近一条记录（有 → 编辑模式预填；无 → 空表单）
    let most_recent_record = sqlx::query_as::<_, Record>(
        "SELECT * FROM records WHERE plan_item_id = ?
        ORDER BY record_date DESC, id DESC LIMIT 1",
    )
    .bind(&item_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(AppError::Database)?;
    // 6. 拼 HTML：计划值 + 上次参考 + 表单（含换算器挂载点）

    // 6a. 上次记录参考（Option → HTML 行，None → 提示"还没有记录"）
    //     【教学：map + unwrap_or_default 链——处理 Option 不用 if】
    //     Some → 拼一行"上次 {日期}：{重量}kg × {组}组 × {次}次，感受：…，策略：…"
    //     None → "还没有记录，这是第一次！"
    let last_ref = most_recent_record
        .as_ref()
        .map(|r| {
            format!(
                "上次 {date}: {weight}kg * {sets}组 * {reps}次<br>\
                 感受：{feeling}<br>策略：{strategy}",
                date = r.record_date,
                weight = r.weight,
                sets = r.sets,
                reps = r.reps,
                feeling = if r.feeling.is_empty()
                {
                    "-".to_string()
                }
                else
                {
                    r.feeling.clone()
                },
                strategy = if r.strategy.is_empty()
                {
                    "-".to_string()
                }
                else
                {
                    r.strategy.clone()
                },
            )
        })
        .unwrap_or_else(|| "还没有记录，这是第一次！".to_string());

    // 6b. 表单预填 —— 预填链：计划预设 → 最近记录 → 动作库默认
    //     【教学：预填链 = 三层 Option 优先级（M6 扩展）】
    //     计划编辑时已能预设计重信息（plan_weight/plan_mode/plan_bar_weight/
    //     plan_rest/plan_key_points），所以 record_form 预填不再
    //     "有记录就全取旧值"，而是按优先级：
    //       1. plan_item 有预设 → 用它（训练前的安排优先，用户按计划执行）
    //       2. 没有 → 最近记录旧值（上次实际完成的参照，渐进超负荷）
    //       3. 再没有 → 动作库默认（新动作第一条）
    //     感受/策略只在 record_form 填（计划层没有这两列）→ 只有 2/3 两层。
    //     or_else 链：Option 依次尝试，第一个 Some 生效，全 None 才落兜底。
    let prefill_weight = plan_item
        .plan_weight
        .map(|v| v.to_string())
        .or_else(|| most_recent_record.as_ref().map(|r| r.weight.to_string()))
        .unwrap_or_default();
    let prefill_sets = plan_item
        .plan_sets
        .map(|v| v.to_string())
        .or_else(|| most_recent_record.as_ref().map(|r| r.sets.to_string()))
        .unwrap_or_default();
    let prefill_reps = plan_item
        .plan_reps
        .map(|v| v.to_string())
        .or_else(|| most_recent_record.as_ref().map(|r| r.reps.to_string()))
        .unwrap_or_default();
    let prefill_rest = plan_item
        .plan_rest
        .map(|v| v.to_string())
        .or_else(|| most_recent_record.as_ref().map(|r| r.rest.to_string()))
        .unwrap_or_default();
    let prefill_feeling = most_recent_record
        .as_ref()
        .map(|r| r.feeling.clone())
        .unwrap_or_default();
    let prefill_strategy = most_recent_record
        .as_ref()
        .map(|r| r.strategy.clone())
        .unwrap_or_default();
    let prefill_key_points = plan_item
        .plan_key_points
        .clone()
        .or_else(|| most_recent_record.as_ref().map(|r| r.key_points.clone()))
        .unwrap_or_else(|| exercise_details.key_points.clone());
    let prefill_mode = plan_item
        .plan_mode
        .clone()
        .or_else(|| most_recent_record.as_ref().map(|r| r.mode.clone()))
        .unwrap_or_else(|| exercise_details.default_mode.clone());

    // 6c. 模式下拉框选项（当前模式 selected，其余普通）
    //     【教学：select 的 selected 由后端决定】
    //     和 M3 exercises.rs 的 mode_options 完全同款：
    //     遍历 4 种模式，当前模式加 " selected"，其余空串。
    let mode_options = ["bar", "support", "std", "lb2kg"]
        .iter()
        .map(|mode| {
            format!(
                r#"<option value="{mode}"{sel}>{mode_name}</option>"#,
                sel = if *mode == prefill_mode
                {
                    " selected"
                }
                else
                {
                    ""
                },
                mode_name = match *mode
                {
                    "bar" => "杠铃",
                    "support" => "支撑",
                    "std" => "标准kg",
                    "lb2kg" => "标准lb",
                    _ => *mode,
                },
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    // 6c-1. 杆重下拉框选项（四种杠铃规格枚举，与 exercises.rs 的 bar_weight 同款）
    //     【教学：杆重不是随便填的数字，是健身房四种杠铃规格之一】
    //     Olympic(20kg) / Smith(11.3kg) / 短杠(10kg) / 双边(0kg) 四选一，
    //     ⚠️ 预填链：plan_item.plan_bar_weight（计划预设）→ 动作 bar_weight（默认）。
    //     为什么优先计划预设？用户编辑计划时可能为某动作指定"今天用 Smith 杆"，
    //     换算器初始杆重应该跟着计划的预设走，而不是动作库的通用默认。
    //     select 的 value 就是选中 option 的 value（数字字符串），
    //     换算器 JS 里 Number(barInput.value) 照常解析（"0" → 0）。
    //     双边(0kg)：倒蹲等无杆动作，两边放片但轴本身不称重，
    //     总重 = 0 + 2 × 片重（和 Olympic 同公式，只是杆重为 0）。
    let prefill_bar_weight = plan_item
        .plan_bar_weight
        .unwrap_or(exercise_details.bar_weight);
    let bar_weight_options = ["20", "11.3", "10", "0"]
        .iter()
        .map(|bar_weight| {
            format!(
                r#"<option value="{bar_weight}"{sel}>{bar_weight_name}</option>"#,
                sel = if *bar_weight == format!("{}", prefill_bar_weight)
                {
                    " selected"
                }
                else
                {
                    ""
                },
                bar_weight_name = match *bar_weight
                {
                    "20" => "Olympic(20kg)",
                    "11.3" => "Smith(11.3kg)",
                    "10" => "短杠(10kg)",
                    "0" => "双边(0kg)",
                    _ => *bar_weight,
                },
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    // 6d. 拼页面
    //     【教学：r#"..."# 里不能有裸 { } —— format! 只认命名参数】
    //     页面底部有 JS（换算器脚本 + 显隐切换），JS 里全是 {}，
    //     所以 JS 字符串用命名参数 {javascript} 单独传入，
    //     避免 format! 把 JS 的 {} 当占位符。
    //     body 挂 data-bar-weight（换算器读初始杆重）。
    Ok(Html(format!(
        r#"<!DOCTYPE html>
        <html lang="zh">
        <head><meta charset="UTF-8"><meta name="viewport" content="width=device-width, initial-scale=1.0"><title>记录：{ex_name}</title></head>
        <body data-bar-weight="{bar_weight}">
        <h2>记录：{ex_name}</h2>        <p>计划值：{plan_sets}组 * {plan_reps}次{plan_weight_text}</p>
        <p>上次参考：{last_ref}</p>

        <form method="post" action="/plans/{plan_id}/record/{item_id}/save">
            <label>实际强度
                <input name="weight" id="weight-input" type="number" step="0.5" value="{prefill_weight}" readonly style="background:#eee; color:#888; cursor:not-allowed;">
            </label><br>

            <label>计重方式
                <select name="mode" id="mode-select">
                    {mode_options}
                </select>
            </label><br>
            <div id="bar-row">
                <label>杆重
                    <select id="bar-input">
                        {bar_weight_options}
                    </select>
                </label>
            </div>
            <div id="body-row" style="display:none">
                <label>体重
                    <input id="body-input" type="number" step="0.5" value="">
                </label>
            </div>
            <label>观测强度
                <input id="plate-input" type="number" step="0.5" value="">
            </label>
            <span id="result"></span>
            <button type="button" id="fill-btn">填入强度</button><br>

            <label>组数
                <input name="sets" type="number" step="1" value="{prefill_sets}">
            </label><br>
            <label>次数
                <input name="reps" type="number" step="1" value="{prefill_reps}">
            </label><br>
            <label>休息（秒）
                <input name="rest" type="number" step="1" value="{prefill_rest}">
            </label><br>
            <label>感受
                <input name="feeling" value="{prefill_feeling}">
            </label><br>
            <label>下次训练策略
                <input name="strategy" value="{prefill_strategy}">
            </label><br>
            <label>要领
                <textarea name="key_points">{prefill_key_points}</textarea>
            </label><br>
            <button type="submit">保存</button>
        </form>
        <p><a href="/today">返回今日</a></p>
        <script>
            {javascript}
        </script>
        <script src="/static/weight_converter.js"></script>
        </body>
        </html>"#,
        ex_name = exercise_details.name,
        bar_weight = prefill_bar_weight,
        plan_sets = plan_item
            .plan_sets
            .map_or("-".to_string(), |v| v.to_string()),
        plan_reps = plan_item
            .plan_reps
            .map_or("-".to_string(), |v| v.to_string()),
        plan_weight_text = plan_item
            .plan_weight
            .map_or(String::new(), |v| format!("（{v}kg）"),),
        last_ref = last_ref,
        plan_id = current_plan.id,
        item_id = plan_item.id,
        prefill_weight = prefill_weight,
        mode_options = mode_options,
        bar_weight_options = bar_weight_options,
        prefill_sets = prefill_sets,
        prefill_reps = prefill_reps,
        prefill_rest = prefill_rest,
        prefill_feeling = prefill_feeling,
        prefill_strategy = prefill_strategy,
        prefill_key_points = prefill_key_points,
        javascript = "function toggleBarWeight() {
            var mode = document.getElementById('mode-select').value;
            document.getElementById('bar-row').style.display =
                (mode === 'bar') ? '' : 'none';
            document.getElementById('body-row').style.display =
                (mode === 'support') ? '' : 'none';
        }
        toggleBarWeight();",
    )))
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
    Path((plan_id, item_id)): Path<(i64, i64)>,
    Form(form): Form<RecordForm>,
) -> Result<Redirect, AppError>
{
    // 1. 签名：State + AuthUser + Path((plan_id, item_id)) + Form(form)
    // 2. 验证归属（同 record_form）
    let current_plan = sqlx::query_as::<_, Plan>(
        "SELECT p.* FROM plans p
        INNER JOIN phases ph ON p.phase_id = ph.id
        WHERE p.id = ? AND ph.user_id = ?",
    )
    .bind(&plan_id)
    .bind(&user.id)
    .fetch_optional(&state.pool)
    .await
    .map_err(AppError::Database)?
    .ok_or_else(|| AppError::NotFound("No plan found in such user and phase".to_string()))?;

    let phase = sqlx::query_as::<_, Phase>("SELECT * FROM phases WHERE id = ? AND user_id = ?")
        .bind(&current_plan.phase_id)
        .bind(&user.id)
        .fetch_optional(&state.pool)
        .await
        .map_err(AppError::Database)?
        .ok_or_else(|| AppError::NotFound("No phase found".to_string()))?;
    if phase.archived
    {
        return Err(AppError::Forbidden(
            "Can not edit archived phase".to_string(),
        ));
    }
    // 2.5 验证计划项属于该计划（双条件防越权）+ 拿 exercise_id
    //     【教学：record_save 和 record_form 必须做同样的归属验证】
    //     不只是"重复代码"问题：POST 可以被绕过前端直接发请求，
    //     不验证 plan_item 属于该 plan，就能拿别人的计划项 id 往自己库里插。
    //     顺便拿到 exercise_id（INSERT 要用）。
    let plan_item =
        sqlx::query_as::<_, PlanItem>("SELECT * FROM plan_items WHERE id = ? AND plan_id = ?")
            .bind(&item_id)
            .bind(&current_plan.id)
            .fetch_optional(&state.pool)
            .await
            .map_err(AppError::Database)?
            .ok_or_else(|| AppError::NotFound("No plan item found".to_string()))?;
    // 3. parse 数字字段：weight → f64，sets/reps/rest → i64
    //    （parse 失败 → Validation；负数 → Validation）
    let weight = form
        .weight
        .parse::<f64>()
        .map_err(|_| AppError::Validation("重量必须是数字".to_string()))?;
    let sets = form
        .sets
        .parse::<i64>()
        .map_err(|_| AppError::Validation("组数必须是数字".to_string()))?;
    let reps = form
        .reps
        .parse::<i64>()
        .map_err(|_| AppError::Validation("次数必须是数字".to_string()))?;
    let rest = form
        .rest
        .parse::<i64>()
        .map_err(|_| AppError::Validation("休息时间必须是数字".to_string()))?;
    // 3.5 负数校验（训练数据不可能是负数）
    if weight < 0.0 || sets < 0 || reps < 0 || rest < 0
    {
        return Err(AppError::Validation(
            "重量/组数/次数/休息不能为负数".to_string(),
        ));
    }
    // 4. 查该计划项最近一条记录（决定 INSERT 还是 UPDATE）
    let most_recent_record = sqlx::query_as::<_, Record>(
        "SELECT * FROM records WHERE plan_item_id = ?
        ORDER BY record_date DESC, id DESC LIMIT 1",
    )
    .bind(&item_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(AppError::Database)?;
    // 5. 有记录 → UPDATE：
    //    UPDATE records SET weight=?, sets=?, reps=?, rest=?,
    //      feeling=?, strategy=?, key_points=?, mode=?
    //    WHERE id = ?（按查到的记录 id）
    // 6. 无记录 → INSERT：
    //    INSERT INTO records
    //      (plan_item_id, phase_id, exercise_id, record_date,
    //       weight, sets, reps, rest, feeling, strategy, key_points, mode)
    //    VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
    //    （phase_id/exercise_id 从计划项 JOIN 取；record_date = 今天）
    match most_recent_record
    {
        Some(record) =>
        {
            sqlx::query(
                "UPDATE records
                SET weight = ?,
                sets = ?,
                reps = ?,
                rest = ?,
                feeling = ?,
                strategy = ?,
                key_points = ?,
                mode = ?
                WHERE id = ?",
            )
            .bind(&weight)
            .bind(&sets)
            .bind(&reps)
            .bind(&rest)
            .bind(&form.feeling)
            .bind(&form.strategy)
            .bind(&form.key_points)
            .bind(&form.mode)
            .bind(&record.id)
            .execute(&state.pool)
            .await
            .map_err(AppError::Database)?;
        },
        None =>
        {
            let today_dt = sqlx::query_scalar::<_, String>("SELECT date('now', 'localtime')")
                .fetch_one(&state.pool)
                .await
                .map_err(AppError::Database)?;
            sqlx::query(
                "INSERT INTO records
                (plan_item_id, phase_id, exercise_id, record_date,
                weight, sets, reps, rest, feeling, strategy, key_points, mode)
                VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            )
            .bind(&plan_item.id)
            .bind(&phase.id)
            .bind(&plan_item.exercise_id)
            .bind(&today_dt)
            .bind(&weight)
            .bind(&sets)
            .bind(&reps)
            .bind(&rest)
            .bind(&form.feeling)
            .bind(&form.strategy)
            .bind(&form.key_points)
            .bind(&form.mode)
            .execute(&state.pool)
            .await
            .map_err(AppError::Database)?;
        },
    }
    // 7. 重定向回 /today（今日页刷新后显示 ✅ 已训练）
    Ok(Redirect::to("/today"))
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
