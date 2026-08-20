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

use crate::{
    AppState,
    error::AppError,
    handlers::auth::AuthUser,
    models::{Exercise, Phase, Plan, PlanItem, Record, group_by_body_part},
};

// ============================================================
// 【M5 修订：计重模式展示串 —— 计划值/上次参考/历史页共用】
// ============================================================
/// 杆重规格 → 显示名（bar 模式括号里用）
pub fn bar_name(bar_weight: f64) -> &'static str
{
    match bar_weight
    {
        20.0 => "Olympic",
        11.3 => "Smith",
        10.0 => "Short",
        _ => "dual", // 0kg 双边（M5 修订：小写）
    }
}

/// 计重模式展示串：`模式(参数, 观测强度)`
///
/// 【教学：观测强度从"实际强度"逆换算】
/// 训练时用户在器械上看到的是**片重/支撑量**（"每边 20kg"），
/// 显示实际总重（60kg）反而不直观——用户要的是器械上的读数。
/// 与 weight_converter.js 的 inverseConvert 同逻辑：
///   bar     → 片重 = (总重 - 杆重) / 2
///   support → 支撑量 = 体重 - 总重
///   std     → 片重 = 总重
/// 【M5 修订：unit 参数 —— 观测强度单位（kg/lb）】
///   lb 单位 → 观测强度显示值 = kg 值 ÷ 0.4536（与换算器互逆）。
///   实际强度始终是 kg（weight 字段），只有观测强度换单位。
/// ⚠️ 已知取舍：support 用当前体重逆换算，改体重后历史记录的
///   观测强度会跟着变——与旧 Python 版行为一致，接受。
pub fn mode_display(
    mode: &str,
    weight: f64,
    bar_weight: f64,
    body_weight: Option<f64>,
    unit: &str,
) -> String
{
    // kg → 显示单位（lb 时 ÷0.4536，与 weight_converter.js 互逆）
    let disp = |kg: f64| {
        if unit == "lb"
        {
            format!("{:.0}lb", (kg / 0.4536).round())
        }
        else
        {
            format!("{kg}kg")
        }
    };
    match mode
    {
        "bar" =>
        {
            let plate = ((weight - bar_weight) / 2.0).max(0.0);
            format!("bar({}, {})", bar_name(bar_weight), disp(plate))
        },
        "support" =>
        {
            let body = body_weight.unwrap_or(0.0);
            let support = (body - weight).max(0.0);
            format!("support({body}kg - {})", disp(support))
        },
        // std（M6 清理：lb2kg 历史值已迁移归正，观测强度 = 总重）
        _ => format!("std({})", disp(weight)),
    }
}

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
    let pool = state.pool.read().await.clone();

    // 1. 签名：State + AuthUser
    // 2. 查进行中阶段：
    //    SELECT * FROM phases WHERE user_id = ? AND archived = 0
    //    ORDER BY created_at DESC LIMIT 1
    //    → 没有 → 空态提示"暂无进行中阶段，请先创建"
    let current_phase = match sqlx::query_as::<_, Phase>(
        "SELECT * FROM phases WHERE user_id = ? AND archived = 0 ORDER BY created_at DESC LIMIT 1",
    )
    .bind(&user.id)
    .fetch_optional(&pool)
    .await
    .map_err(AppError::Database)?
    {
        Some(p) => p,
        // 【M7 第 4 步：空态 —— 无进行中阶段 → 友好提示页（原来是 404）】
        None =>
        {
            return Ok(Html(
                format!(
                    r#"{head}
            <h2>今日训练</h2>
            <p class="empty-tip">暂无进行中阶段，请先创建</p>
            <p><a href="/phases/new">去创建阶段</a></p>
            <p><a href="/">返回首页</a></p>"#,
                    head = crate::page::page_head("今日训练"),
                )
                .to_string(),
            ));
        },
    };
    // 3. 查今天：SELECT date('now', 'localtime')
    let today_dt = sqlx::query_scalar::<_, String>("SELECT date('now', 'localtime')")
        .fetch_one(&pool)
        .await
        .map_err(AppError::Database)?;
    // 4. 查今天的计划：
    //    SELECT * FROM plans WHERE phase_id = ? AND date = ?
    //    → 没有 → 空态提示"今天还没有计划"
    let today_plan =
        match sqlx::query_as::<_, Plan>("SELECT * FROM plans WHERE phase_id = ? AND date = ?")
            .bind(&current_phase.id)
            .bind(&today_dt)
            .fetch_optional(&pool)
            .await
            .map_err(AppError::Database)?
        {
            Some(p) => p,
            // 【M7 第 4 步：空态 —— 今天无计划 → 友好提示页（原来是 404）】
            None =>
            {
                return Ok(Html(
                    format!(
                        r#"{head}
            <h2>今日训练({today_dt})</h2>
            <p class="empty-tip">今天还没有计划</p>
            <p><a href="/plans/new?phase_id={phase_id}">去新建今日计划</a></p>
            <p><a href="/">返回首页</a></p>"#,
                        head = crate::page::page_head("今日训练"),
                        today_dt = today_dt,
                        phase_id = current_phase.id,
                    )
                    .to_string(),
                ));
            },
        };
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
    .fetch_all(&pool)
    .await
    .map_err(AppError::Database)?;
    let id_to_ex = sqlx::query_as::<_, Exercise>("SELECT * FROM exercises WHERE user_id = ?")
        .bind(&user.id)
        .fetch_all(&pool)
        .await
        .map_err(AppError::Database)?
        .iter()
        // 【M4 修订：索引扩展成 (动作名, 身体部位) 元组】
        // today 页要按 body_part 分组，所以索引不再只存名字
        // 【M5 修订：再加 (默认模式, 默认杆重, 默认单位) —— 计划值展示需要】
        .map(|e| {
            (
                e.id,
                (
                    e.name.clone(),
                    e.body_part.clone(),
                    e.default_mode.clone(),
                    e.bar_weight,
                    e.default_unit.clone(),
                ),
            )
        })
        .collect::<HashMap<i64, (String, String, String, f64, String)>>();
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
        .fetch_optional(&pool)
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
        .fetch_one(&pool)
        .await
        .map_err(AppError::Database)?
        .to_string(),
        None => "未设置开始日期".to_string(),
    };

    // 7b. 动作列表行（items_with_records = (计划项, 最近记录) 配对）
    //     【M4 修订：按身体部位分组 + 组间标准排序】
    //     遍历 items_with_records，同一 body_part 的动作归到一组
    //     （保序分组：Vec<(部位, Vec<行>)>，按出现顺序，不重排）。
    //     组内顺序 = plan_items 的 sort_order（上面查询已排好）。
    //     组间顺序 = models::BODY_PART_ORDER 常量（背→胸→肩→核心→腿→手臂），
    //     与模板编辑页/计划详情页一致（未收录部位按出现顺序兜底）。
    //     每个部位渲染一个小节：<h3>部位</h3> + 表格。
    let mut groups: Vec<(String, String)> = Vec::new();
    for (item, last) in &items_with_records
    {
        // 动作名 + 部位 + 默认计重：从索引取（查不到显示 "?" / "未分组"，理论不发生）
        let (ex_name, body_part, ex_default_mode, ex_default_bar, ex_default_unit) =
            id_to_ex.get(&item.exercise_id).cloned().unwrap_or_else(|| {
                (
                    "?".to_string(),
                    "未分组".to_string(),
                    "std".to_string(),
                    0.0,
                    "kg".to_string(),
                )
            });
        let ex_defaults = (ex_default_mode, ex_default_bar, ex_default_unit);
        // 计划值：模式(参数,观测强度), sets*reps
        // 【M6 修订：计重方式/杆重/单位统一取 exercise 默认】
        // plan_mode/plan_bar_weight 已废弃（models.rs），
        // 计划项不再维护计重配置——单一事实来源 = 动作库。
        let plan_value = {
            let plan_mode = ex_defaults.0.clone();
            // 【M5 修订：support 无重量 → 回退体重（自重动作）】
            // 引体向上等自重动作不填计划重量 = "无辅助引体"，
            // 实际负重 = 体重、支撑 = 0。None 当 0 会显示"支撑 90kg"（错）。
            let plan_weight = item.plan_weight.unwrap_or_else(|| {
                if plan_mode == "support"
                {
                    user.body_weight.unwrap_or(0.0)
                }
                else
                {
                    0.0
                }
            });
            let plan_bar = ex_defaults.1;
            format!(
                "{mode} | {sets}*{reps}",
                mode = mode_display(
                    &plan_mode,
                    plan_weight,
                    plan_bar,
                    user.body_weight,
                    &ex_defaults.2,
                ),
                sets = item.plan_sets.map_or("-".to_string(), |v| v.to_string()),
                reps = item.plan_reps.map_or("-".to_string(), |v| v.to_string()),
            )
        };
        // 整行状态色 + 上次策略提示
        // 【M5 修订：整行填色替代状态列（信息更直观、少一列更紧凑）】
        //   None                → 灰色   #dddddd（未训练）
        //   Some(rec) 未勾选    → 黄色   #ffe08a（已记录未完成）
        //   Some(rec) completed → 绿色   #b7e4b0（已完成）
        //   tr 加 style 背景色，所有 td 一起变色（无位移风险）
        // 【M5 修订：策略去掉"上次策略："前缀——列头已表明含义】
        let (status_color, strategy_hint) = match last
        {
            Some(rec) if rec.completed => ("#b7e4b0", rec.strategy.clone()),
            Some(rec) => ("#ffe08a", rec.strategy.clone()),
            None => ("#dddddd", String::new()),
        };
        let row = format!(
            "<tr style=\"background-color:{status_color}\"><td>{ex_name}</td><td>{plan_value}</td>\
             <td>{strategy_hint}</td>\
             <td><a href=\"/plans/{plan_id}/record/{item_id}\">记录/编辑</a></td></tr>",
            ex_name = ex_name,
            plan_value = plan_value,
            status_color = status_color,
            strategy_hint = strategy_hint,
            plan_id = today_plan.id,
            item_id = item.id,
        );
        // 保序分组 → 组间标准排序：group_by_body_part 返回
        //   按配置顺序排好的 (部位, 行列表)，组内保持本处顺序
        groups.push((body_part, row));
    }
    // 统一交给 group_by_body_part（保序分组 + 组间配置排序）
    // 【M4 修订：顺序来自 AppConfig.body_part_order（环境变量可配）】
    let groups = group_by_body_part(groups.into_iter(), &state.config.body_part_order);

    // 7c. 每组渲染一个小节（<h3>部位名</h3> + 表头 + 行）
    let grouped_html = groups
        .iter()
        .map(|(part, rows)| {
            format!(
                "<h3>{part}</h3>\n<table border=\"1\"><tr><th>动作</th><th>计划值</th><th>上次策略</th><th>操作</th></tr>\n{rows}\n</table>",
                part = part,
                rows = rows.join("\n"),
            )
        })
        .collect::<Vec<String>>()
        .join("\n");

    // 7d. 拼整页（风格与 M3 一致：h2 + 分组小节 + 返回链接）
    // 【M6：PWA 注入 —— manifest + Service Worker 注册】
    Ok(Html(format!(
        r#"
        {head}
        <h2>今日训练({today_dt})</h2>
        <p>阶段：{phase_name} | 已坚持 {persist_days} 天</p>
        {grouped_html}
        <p><a href="/">返回首页</a></p>
        "#,
        head = crate::page::page_head("今日训练"),
        today_dt = today_dt,
        phase_name = current_phase.name,
        persist_days = persist_days,
        grouped_html = grouped_html,
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
///   <select id="mode-select">（bar/support/std，默认动作的 default_mode）
///   <select id="unit-select">（kg/lb，观测强度单位，M4 新增）
///   <input id="plate-input">     片重/支撑量
///   <input id="bar-input">       杆重（bar 模式才显示）
///   <input id="body-input">     体重（support 模式才显示）
///   <span id="result">           换算结果
///   <input id="weight-input">   实际强度（readonly，JS 实时自动写入）
///   页面 <body data-bar-weight="动作.bar_weight"> 提供初始杆重
/// 三种模式公式（与 JS 一致）：
///   bar     = 杆重 + 2×片重
///   support = 体重 − 支撑量（支撑器械标的是"抵消多少体重"，
///             如 90kg 体重 + 30kg 支撑做引体 → 实际负重 60kg）
///   std     = 片重
/// 观测强度带单位：选 lb 时先 ×0.4536 归一化成 kg 再套公式（M4 修订）
pub async fn record_form(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path((plan_id, item_id)): Path<(i64, i64)>,
) -> Result<Html<String>, AppError>
{
    let pool = state.pool.read().await.clone();

    // 1. 签名：State + AuthUser + Path((plan_id, item_id))
    // 2. 验证计划归属：JOIN phases 查 user_id
    let current_plan = sqlx::query_as::<_, Plan>(
        "SELECT p.* FROM plans p
        INNER JOIN phases ph ON p.phase_id = ph.id
        WHERE p.id = ? AND ph.user_id = ?",
    )
    .bind(&plan_id)
    .bind(&user.id)
    .fetch_optional(&pool)
    .await
    .map_err(AppError::Database)?
    .ok_or_else(|| AppError::NotFound("No plan found in such user and phase".to_string()))?;

    let phase = sqlx::query_as::<_, Phase>("SELECT * FROM phases WHERE id = ? AND user_id = ?")
        .bind(&current_plan.phase_id)
        .bind(&user.id)
        .fetch_optional(&pool)
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
            .fetch_optional(&pool)
            .await
            .map_err(AppError::Database)?
            .ok_or_else(|| AppError::NotFound("No plan item found".to_string()))?;

    // 4. 查动作信息（拿 key_points 预填 + bar_weight 给换算器）
    let exercise_details =
        sqlx::query_as::<_, Exercise>("SELECT * FROM exercises WHERE id = ? AND user_id = ?")
            .bind(&plan_item.exercise_id)
            .bind(&user.id)
            .fetch_optional(&pool)
            .await
            .map_err(AppError::Database)?
            .ok_or_else(|| AppError::NotFound("No such exercise".to_string()))?;
    // 5. 查记录（拆两个查询：当日值 + 上次训练值）
    //     【M5 修订：预填链升级 —— 当日值 > 计划值 > 上次训练值】
    //     旧版只查"最近一条"（可能=今天），且计划值优先于它。
    //     新版拆成两个查询，语义更清晰：
    //       - today_record：今天已保存的记录（训练中改过就回显"今天的实际"）
    //       - last_record：今天之前的最近一条（渐进超负荷参照物 + 兜底回显）
    let today_record = sqlx::query_as::<_, Record>(
        "SELECT * FROM records WHERE plan_item_id = ? AND record_date = date('now','localtime')
        ORDER BY id DESC LIMIT 1",
    )
    .bind(&item_id)
    .fetch_optional(&pool)
    .await
    .map_err(AppError::Database)?;
    // 【M5 修订 bug：上次训练按 exercise_id 查，不按 plan_item_id！】
    // 计划项会因"先删后插"重建（新 id），历史记录挂旧 id——
    // 按当前 plan_item_id 查"上一次"→ 跨计划就查不到（显示"第一次训练"）。
    // 按 exercise_id 查 = "这个动作上次练是什么时候"，跨计划项正确。
    let last_record = sqlx::query_as::<_, Record>(
        "SELECT * FROM records WHERE exercise_id = ? AND record_date < date('now','localtime')
        ORDER BY record_date DESC, id DESC LIMIT 1",
    )
    .bind(&plan_item.exercise_id)
    .fetch_optional(&pool)
    .await
    .map_err(AppError::Database)?;
    // 兼容变量：最近一次记录（当日 → 上次），供"上次参考"展示和 completed 回显
    let most_recent_record = today_record.as_ref().or(last_record.as_ref());
    // 6. 拼 HTML：计划值 + 上次参考 + 表单（含换算器挂载点）

    // 6a. 上次记录参考（Option → HTML 行，None → 提示"还没有记录"）
    //     【教学：map + unwrap_or_default 链——处理 Option 不用 if】
    //     【M5 修订：格式 —— 上次 {date} | {模式(参数,观测强度)} | {sets}*{reps}】
    //     【M5 修订：杆重来源 = 动作库默认值（不是计划项预设）】
    //      records 表不存 bar_weight。训练时实际用的杆无法从记录还原，
    //     展示口径统一用**动作库当前默认**（用户在动作库里维护的标准配置）。
    //     用计划项预设的问题是：上次训练可能挂在不同计划项下，
    //     预设不同 → 同一个动作上次显示 Olympic、这次显示 dual，不一致。
    //     None → "还没有记录，这是第一次！"
    let last_ref = most_recent_record
        .as_ref()
        .map(|r| {
            format!(
                "{date} | {mode_disp} | {sets}*{reps}<br>\
                 感受：{feeling}<br>策略：{strategy}",
                date = r.record_date,
                mode_disp = mode_display(
                    &r.mode,
                    r.weight,
                    exercise_details.bar_weight,
                    user.body_weight,
                    &exercise_details.default_unit,
                ),
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

    // 6b. 表单预填 —— 预填链：当日值 > 计划预设 → 最近记录 → 动作库默认
    //     【教学：预填链 = 四层 Option 优先级（M5 修订）】
    //     计划编辑时已能预设计重信息（plan_weight/plan_mode/plan_bar_weight/
    //     plan_rest/plan_key_points），所以 record_form 预填不再
    //     "有记录就全取旧值"，而是按优先级：
    //       0. 当日值（今天已保存过）→ 训练中"改过再进来"回显今天改的值
    //       1. plan_item 有预设 → 用它（训练前的安排优先，用户按计划执行）
    //       2. 没有 → 上次记录旧值（上次实际完成的参照，渐进超负荷）
    //       3. 再没有 → 动作库默认（新动作第一条）
    //     感受/策略只在 record_form 填（计划层没有这两列）→ 只有 0/2/3 三层。
    //     or_else 链：Option 依次尝试，第一个 Some 生效，全 None 才落兜底。
    let prefill_weight = today_record
        .as_ref()
        .map(|r| r.weight.to_string())
        .or_else(|| plan_item.plan_weight.map(|v| v.to_string()))
        .or_else(|| last_record.as_ref().map(|r| r.weight.to_string()))
        .unwrap_or_default();
    let prefill_sets = today_record
        .as_ref()
        .map(|r| r.sets.to_string())
        .or_else(|| plan_item.plan_sets.map(|v| v.to_string()))
        .or_else(|| last_record.as_ref().map(|r| r.sets.to_string()))
        .unwrap_or_default();
    let prefill_reps = today_record
        .as_ref()
        .map(|r| r.reps.to_string())
        .or_else(|| plan_item.plan_reps.map(|v| v.to_string()))
        .or_else(|| last_record.as_ref().map(|r| r.reps.to_string()))
        .unwrap_or_default();
    let prefill_rest = today_record
        .as_ref()
        .map(|r| r.rest.to_string())
        .or_else(|| plan_item.plan_rest.map(|v| v.to_string()))
        .or_else(|| last_record.as_ref().map(|r| r.rest.to_string()))
        .unwrap_or_default();
    let prefill_feeling = today_record
        .as_ref()
        .map(|r| r.feeling.clone())
        .unwrap_or_default();
    // 【M7 bugfix：策略/感受不再从 last_record 预填】
    // 今天还没训练时（today_record = None），上次的感受/策略是"上次训练"的
    // 旧内容——预填会让用户误以为今天填过。这两个字段只回显当日值，
    // 没填过就是空白，由用户当次输入。
    let prefill_strategy = today_record
        .as_ref()
        .map(|r| r.strategy.clone())
        .unwrap_or_default();
    let prefill_key_points = today_record
        .as_ref()
        .map(|r| r.key_points.clone())
        .or_else(|| plan_item.plan_key_points.clone())
        .or_else(|| last_record.as_ref().map(|r| r.key_points.clone()))
        .unwrap_or_else(|| exercise_details.key_points.clone());
    // 【M6 修订：mode 预填链 —— 当日记录 → 动作默认（PlanItem 不再维护）】
    // plan_mode 已废弃：训练中改过（today_record.mode）→ 回显今天的；
    // 没改过 → 动作库 default_mode。
    let prefill_mode = today_record
        .as_ref()
        .map(|r| r.mode.clone())
        .or_else(|| last_record.as_ref().map(|r| r.mode.clone()))
        .unwrap_or_else(|| exercise_details.default_mode.clone());

    // 6c. 模式下拉框选项（当前模式 selected，其余普通）
    //     【教学：select 的 selected 由后端决定】
    //     （M6 清理：lb2kg 历史值已迁移归正，直接使用）
    let mode_options = ["bar", "support", "std"]
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
                    "std" => "标准",
                    _ => *mode,
                },
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    // 6c-1. 杆重下拉框选项（四种杠铃规格枚举，与 exercises.rs 的 bar_weight 同款）
    //     【教学：杆重不是随便填的数字，是健身房四种杠铃规格之一】
    //     Olympic(20kg) / Smith(11.3kg) / 短杠(10kg) / 双边(0kg) 四选一，
    //     【M6 修订：预填直接取动作库默认 bar_weight（PlanItem 不再维护）】
    //     select 的 value 就是选中 option 的 value（数字字符串），
    //     换算器 JS 里 Number(barInput.value) 照常解析（"0" → 0）。
    //     双边(0kg)：倒蹲等无杆动作，两边放片但轴本身不称重，
    //     总重 = 0 + 2 × 片重（和 Olympic 同公式，只是杆重为 0）。
    let prefill_bar_weight = exercise_details.bar_weight;
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

    // 6c-2. 【M5 修订：全局体重（users.body_weight，首页维护）】
    //     用户问题 0：support 模式的体重应来自"可编辑的通用变量"，
    //     在首页统一维护，record_form 自动获取——不再只靠 localStorage。
    //     localStorage 是浏览器端记忆（换设备就丢），
    //     users 表是"用户属性"的归属地（display_name 同款）。
    //     AuthUser 提取器已 SELECT u.* 全列，body_weight 直接可读。
    let prefill_body_weight = user.body_weight.map(|v| v.to_string()).unwrap_or_default();
    //     兜底：未设置体重时换算器用 70kg 默认（weight_converter.js 内置），
    //     页面显示"未设置"提示（见 body-row 的 data 属性）。

    // 6d. 拼页面
    //     【教学：r#"..."# 里不能有裸 { } —— format! 只认命名参数】
    //     页面底部有 JS（换算器脚本 + 显隐切换），JS 里全是 {}，
    //     所以 JS 字符串用命名参数 {javascript} 单独传入，
    //     避免 format! 把 JS 的 {} 当占位符。
    //     body 挂 data-bar-weight（换算器读初始杆重）。
    // 【M5 修订：记录页嵌入最近 180 天趋势图（stats.rs 公共函数复用）】
    //     位置：动作名称下面、上次参考上面。
    //     None（< 2 条记录）→ 表单页不放"记录太少"文案，静默省略图。
    let chart_section =
        match crate::handlers::stats::exercise_chart_html(&pool, exercise_details.id).await?
        {
            Some(html) => html,
            None => String::new(),
        };
    Ok(Html(format!(
        r#"<!DOCTYPE html>
        <html lang="zh">
        {head}
        <body data-bar-weight="{bar_weight}">
        <h2>记录：{ex_name}</h2>        {plan_note_block}{action_note_block}{chart_section}<p>计划值：{plan_value_display}</p>
        <p>上次参考：{last_ref}</p>

        <form method="post" action="/plans/{plan_id}/record/{item_id}/save">
            <label>已完成
                <input type="checkbox" name="completed" value="1"{completed_checked}>
            </label><br>

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
                    <select id="bar-input" name="bar_weight">
                        {bar_weight_options}
                    </select>
                </label>
            </div>
            <div id="body-row" style="display:none">
                <label>体重
                    <input id="body-input" type="number" step="0.5" value="{prefill_body_weight}" placeholder="未设置">
                </label>
            </div>
            <label>观测强度
                <input id="plate-input" type="number" step="0.5" value="">
                <select id="unit-select" name="unit">
                    {unit_options}
                </select>
            </label>
            <span id="result"></span><br>

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
        <script src="/static/weight_converter.js?v=4"></script>
        </body>
        </html>"#,
        head = crate::page::page_head("记录"),
        ex_name = exercise_details.name,
        bar_weight = prefill_bar_weight,
        prefill_body_weight = prefill_body_weight,
        chart_section = chart_section,
        // 【M5 修订：展示计划备注（plans.note）作为训练提醒】
        // 用户建计划时可以写"xxkg晋级赛"、"加深动作行程"这类提醒，
        // 训练时应在记录页看到。有内容才渲染该行，空备注不占位。
        plan_note_block = if current_plan.note.is_empty()
        {
            String::new()
        }
        else
        {
            format!(
                r#"<p style="color:#c00;font-weight:bold">计划备注：{note}</p>"#,
                note = current_plan.note
            )
        },
        // 【M5 修订：动作级备注（plan_items.plan_note，0004 迁移）】
        // 区别于 plans.note（整计划备注）：这里是该动作自己的训练提醒，
        // 如"深蹲行程深一些"、"硬拉70kg晋级赛"。有内容才渲染，空不占位。
        action_note_block = plan_item
            .plan_note
            .clone()
            .filter(|n| !n.is_empty())
            .map(|n| {
                format!(
                    r#"<p style="color:#c60;font-weight:bold">动作备注：{note}</p>"#,
                    note = n
                )
            })
            .unwrap_or_default(),
        plan_value_display = format!(
            "{mode} | {sets}*{reps}",
            mode = mode_display(
                // 【M6 修订：计重三字段直接取 exercise 默认（PlanItem 不再维护）】
                &exercise_details.default_mode,
                // 【M5 修订：support 无重量 → 回退体重（自重动作）】
                plan_item.plan_weight.unwrap_or_else(|| {
                    if exercise_details.default_mode == "support"
                    {
                        user.body_weight.unwrap_or(0.0)
                    }
                    else
                    {
                        0.0
                    }
                }),
                exercise_details.bar_weight,
                user.body_weight,
                &exercise_details.default_unit,
            ),
            sets = plan_item
                .plan_sets
                .map_or("-".to_string(), |v| v.to_string()),
            reps = plan_item
                .plan_reps
                .map_or("-".to_string(), |v| v.to_string()),
        ),
        // 【M5 修订：单位下拉预填 —— 动作 default_unit 决定初始选中】
        // 动作默认 lb（如美式器械）→ 下拉默认 lb；默认 kg → kg。
        // localStorage 偏好（用户手动切换）仍会覆盖此预填（JS 层处理）。
        unit_options = ["kg", "lb"]
            .iter()
            .map(|u| {
                format!(
                    r#"<option value="{u}"{sel}>{u}</option>"#,
                    sel = if *u == exercise_details.default_unit
                    {
                        " selected"
                    }
                    else
                    {
                        ""
                    },
                )
            })
            .collect::<Vec<String>>()
            .join("\n"),
        last_ref = last_ref,
        plan_id = current_plan.id,
        item_id = plan_item.id,
        // 【M7 bugfix：已完成勾选框只回显当日值】
        // 今天还没训练（today_record = None）时不再从 last_record 预填——
        // 上次训练勾没勾跟今天无关，新记录默认未勾选。
        completed_checked = if today_record.as_ref().map(|r| r.completed).unwrap_or(false)
        {
            " checked"
        }
        else
        {
            ""
        },
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
            // M6 修订：计重配置选择即同步动作库。
            // 用户改模式/杆重/单位 → 立即 POST 到 /exercises/{id}/config
            // 不用等保存记录——改了就生效（单一事实来源 = 动作库）。
            var cfg = new URLSearchParams();
            cfg.set('mode', mode);
            cfg.set('bar_weight', document.getElementById('bar-input').value);
            cfg.set('unit', document.getElementById('unit-select').value);
            fetch('/exercises/{ex_id}/config', {{
                method: 'POST',
                headers: {{ 'Content-Type': 'application/x-www-form-urlencoded' }},
                body: cfg.toString()
            }});
        }
        function syncUnit() {
            var cfg = new URLSearchParams();
            cfg.set('mode', document.getElementById('mode-select').value);
            cfg.set('bar_weight', document.getElementById('bar-input').value);
            cfg.set('unit', document.getElementById('unit-select').value);
            fetch('/exercises/{ex_id}/config', {{
                method: 'POST',
                headers: {{ 'Content-Type': 'application/x-www-form-urlencoded' }},
                body: cfg.toString()
            }});
        }
        document.getElementById('bar-input').addEventListener('input', toggleBarWeight);
        document.getElementById('unit-select').addEventListener('input', syncUnit);
        document.getElementById('mode-select').addEventListener('input', toggleBarWeight);
        toggleBarWeight();"
            .replace("{ex_id}", &exercise_details.id.to_string()),
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
    let pool = state.pool.read().await.clone();

    // 1. 签名：State + AuthUser + Path((plan_id, item_id)) + Form(form)
    // 2. 验证归属（同 record_form）
    let current_plan = sqlx::query_as::<_, Plan>(
        "SELECT p.* FROM plans p
        INNER JOIN phases ph ON p.phase_id = ph.id
        WHERE p.id = ? AND ph.user_id = ?",
    )
    .bind(&plan_id)
    .bind(&user.id)
    .fetch_optional(&pool)
    .await
    .map_err(AppError::Database)?
    .ok_or_else(|| AppError::NotFound("No plan found in such user and phase".to_string()))?;

    let phase = sqlx::query_as::<_, Phase>("SELECT * FROM phases WHERE id = ? AND user_id = ?")
        .bind(&current_plan.phase_id)
        .bind(&user.id)
        .fetch_optional(&pool)
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
            .fetch_optional(&pool)
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
    // 【M4 修订：已完成标记】
    // checkbox 勾选 → form.completed = Some("1") → true；未勾选 → None → false
    let completed = form.completed.as_deref() == Some("1");
    // 4. 查该计划项最近一条记录（决定 INSERT 还是 UPDATE）
    let most_recent_record = sqlx::query_as::<_, Record>(
        "SELECT * FROM records WHERE plan_item_id = ?
        ORDER BY record_date DESC, id DESC LIMIT 1",
    )
    .bind(&item_id)
    .fetch_optional(&pool)
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
                SET completed = ?,
                weight = ?,
                sets = ?,
                reps = ?,
                rest = ?,
                feeling = ?,
                strategy = ?,
                key_points = ?,
                mode = ?
                WHERE id = ?",
            )
            .bind(completed)
            .bind(&weight)
            .bind(&sets)
            .bind(&reps)
            .bind(&rest)
            .bind(&form.feeling)
            .bind(&form.strategy)
            .bind(&form.key_points)
            .bind(&form.mode)
            .bind(&record.id)
            .execute(&pool)
            .await
            .map_err(AppError::Database)?;
        },
        None =>
        {
            let today_dt = sqlx::query_scalar::<_, String>("SELECT date('now', 'localtime')")
                .fetch_one(&pool)
                .await
                .map_err(AppError::Database)?;
            sqlx::query(
                "INSERT INTO records
                (plan_item_id, phase_id, exercise_id, record_date, completed,
                weight, sets, reps, rest, feeling, strategy, key_points, mode)
                VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            )
            .bind(&plan_item.id)
            .bind(&phase.id)
            .bind(&plan_item.exercise_id)
            .bind(&today_dt)
            .bind(completed)
            .bind(&weight)
            .bind(&sets)
            .bind(&reps)
            .bind(&rest)
            .bind(&form.feeling)
            .bind(&form.strategy)
            .bind(&form.key_points)
            .bind(&form.mode)
            .execute(&pool)
            .await
            .map_err(AppError::Database)?;
        },
    }
    // 6.5 【M5 修订：用户诉求 0 —— 记录里的要领回写动作库】
    //     record_form 的要领预填自动作库（key_points），训练时用户常会
    //     边练边修正（"行程深一些"、"手腕中立"），这些修正比动作库的
    //     通用描述更贴合本人，保存记录时同步回 exercises.key_points。
    //     下次任何计划用这个动作，要领都是更新后的版本。
    //     ⚠️ 只回写非空值：空字符串说明用户清空了（有意或无意），
    //        不覆盖动作库的默认描述。
    //     ⚠️ 数据隔离：WHERE id = ? AND user_id = ?（按 id 查询必须带 user_id）
    if !form.key_points.trim().is_empty()
    {
        sqlx::query("UPDATE exercises SET key_points = ? WHERE id = ? AND user_id = ?")
            .bind(&form.key_points)
            .bind(&plan_item.exercise_id)
            .bind(&user.id)
            .execute(&pool)
            .await
            .map_err(AppError::Database)?;
    }
    // 6.6 【M6 修订：计重方式/杆重/单位回写动作库】
    //     record_form 表单里有三个"配置类"字段不存记录表，
    //     但用户在那里改了就是想要长期生效（动作的标准配置）。
    //     保存时同步回 exercises：
    //       - mode（form.mode）→ default_mode
    //       - 杆重（bar-input，前端提交为 hidden 字段 bar_weight）→ bar_weight
    //       - 单位（unit-select，前端提交为 hidden 字段 unit）→ default_unit
    //     ⚠️ 杆重/单位不在 RecordForm 里，需要加字段（见结构体）。
    sqlx::query(
        "UPDATE exercises SET default_mode = ?, bar_weight = ?, default_unit = ?
        WHERE id = ? AND user_id = ?",
    )
    .bind(&form.mode)
    .bind(&form.bar_weight)
    .bind(&form.unit)
    .bind(&plan_item.exercise_id)
    .bind(&user.id)
    .execute(&pool)
    .await
    .map_err(AppError::Database)?;
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
    /// 【M4 修订：已完成勾选框（表单第一个字段）】
    /// checkbox 勾选 → 提交 completed=1 → Some("1")；未勾选 → 键不提交 → None
    /// 只有 completed = 1 时，今日页该动作才标"✅已完成"
    pub completed: Option<String>,
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
    /// 录入时模式（bar/support/std）
    pub mode: String,
    /// 【M6 修订：杆重（bar-input 的当前值，hidden 提交）】
    /// 保存时回写动作库 default 杆重（bar_weight）
    pub bar_weight: String,
    /// 【M6 修订：观测强度单位（unit-select 的当前值，hidden 提交）】
    /// 保存时回写动作库 default_unit
    pub unit: String,
}

// ============================================================
// 【教学：解析表单数字的辅助函数 —— 空串 → 默认值】
// ============================================================
// 【M7 清理：parse_or 已删除】
// 概念教学保留：表单里 sets/reps/rest 用户可能留空，
// 解析失败时需要一个"回落默认值"的写法：
//   s.trim().parse::<i64>().unwrap_or(default)
// 本项目实际各 handler 都是内联写 unwrap_or，没有复用这个泛型函数，
// 留着一个"从未调用"的函数会触发 dead_code 警告，故删除。
// 若 M8+ 发现解析逻辑重复 3 次以上，再抽回泛型函数。
