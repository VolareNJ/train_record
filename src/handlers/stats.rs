// ============================================================
// handlers/stats.rs —— 历史回顾（只读查询 + 图表）
// ============================================================
// 【教学说明】
// 这个文件是 M5 的核心：把 M4 记下的训练数据"读出来、算出来、画出来"。
// 和之前所有 handler 不同，本文件几乎**只读不写**（没有 INSERT/UPDATE），
// 所以不需要事务、不需要表单解析——三个函数都是"查 → 算 → 拼 HTML"。
//
// 三个 handler 对应三张页面（层层下钻）：
//   GET /history                → 历史首页（日历 + 训练日列表）
//   GET /history/{date}         → 某天全部记录
//   GET /exercises/{id}/stats   → 某动作全部历史 + 重量/1RM 折线图 ★ 重点
//
// 下钻关系：
//   历史首页点某天 → 当天详情点某动作 → 动作详情（趋势图）
//
// 📌 阶段要求：M5 你来实现本文件所有函数。
//   实现完成后对照检查（完整实现备份在 docs/learning_path/M5_ref/）。
//
// ⚠️ 接线提醒（本文件写完后）：
//   1. src/handlers/mod.rs 加一行：pub mod stats;
//   2. src/main.rs 注册三条路由（见 M5.md 第 2~4 步）
//   3. src/main.rs 的 home 加历史入口链接（M5.md 第 5 步）
// ============================================================

// 【教学：本文件用到的导入】
// 和 record.rs（M4）对比：
//   - 不需要 Form（没有表单提交）
//   - 新增 Json 不需要（不返回 API JSON），但需要 serde_json::to_string
//     （把图表数据序列化成字符串注入页面 JS 变量）
//   - 新增 Path<String>（日期是字符串 'YYYY-MM-DD'，不是 i64）
//   - HashMap：动作 id → 名字索引（M3/M4 同款模式）
//   - calc::{epley_1rm, wathan_mrm}：M5 第 1 步写的纯函数，这里用上
use std::collections::HashMap;

use axum::{
    extract::{Path, Query, State},
    response::Html,
};
use serde::Deserialize;
use serde_json::json;
use sqlx::SqlitePool;

use crate::{
    AppState,
    calc::{epley_1rm, wathan_mrm},
    error::AppError,
    handlers::auth::AuthUser,
    models::{Exercise, Record},
};

// ============================================================
// 【教学：M5 核心认知 —— "记录"的两种查询口径】
// ============================================================
// M4 的 today 页按 plan_item_id 查记录（查"今天这个计划项练没练"），
// 但 M5 的历史回顾**不按计划查**，只按：
//   1. record_date（日期）——"某天练了哪些动作"
//   2. exercise_id（动作）——"这个动作练过几次、趋势如何"
//
// 为什么？历史回顾不关心"记录来自哪个计划"（计划可能已删除），
// 只关心"事实"：哪天练了什么、表现如何。
// （还记得 M4_bugfix_notes §11 吗？plan_item_id 可能为 NULL，
//  但 M5 的查询不依赖它，所以孤儿记录也能正常统计 ✅）

// ============================================================
// 第一部分：历史首页（GET /history）
// ============================================================
/// 历史首页：年月导航日历（有记录的日子有标记）+ 按动作查看（勾选项）
///
/// 【教学：页面结构】
///   年月下拉（默认当前年月）→ 日历（7 列网格，有记录的日子 ● 链接）
///   勾选"按动作查看"→ 部位筛选 + 动作列表（链接到动作详情）
///
/// 实现步骤：
/// 1. 签名：State + AuthUser + Query（?year=2026&month=08，均可空）
/// 2. 查当前用户全部非空训练日（倒序）
/// 3. 目标年月：query 传了就按 query（校验），没传用当前年月
/// 4. 查目标月天数（SQLite 日期运算）+ 当月有记录的日期集合
/// 5. 渲染日历：有记录的日子 ● 链接；年月下拉 selected 目标值
/// 6. 按动作查看：checkbox 控制显隐（默认收起）+ 部位筛选
/// 7. 空态：一条记录都没有 → 引导去今日页
pub async fn history(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Query(query): Query<CalQuery>,
) -> Result<Html<String>, AppError>
{
    let non_empty_train_dts = sqlx::query_scalar::<_, String>(
        "SELECT DISTINCT record_date FROM records r
        INNER JOIN exercises e ON r.exercise_id = e.id
        WHERE e.user_id = ? ORDER BY record_date DESC",
    )
    .bind(&user.id)
    .fetch_all(&state.pool)
    .await
    .map_err(AppError::Database)?;

    // 空态：一条记录都没有 → 引导去今日页
    if non_empty_train_dts.is_empty()
    {
        return Ok(Html(
            r#"<head><meta name="viewport" content="width=device-width, initial-scale=1.0"></head>
            <h2>历史回顾</h2>
            <p>还没有训练记录，去<a href="/today">今日页</a>开始第一次训练吧</p>
            <p><a href="/">返回首页</a></p>"#
                .to_string(),
        ));
    }

    // 【M5 修订：目标年月 —— query 参数优先，默认当前年月】
    //   ?year=2025&month=03 → 看 2025-03 的日历（导航历史）
    //   无参数 → 当前年月（默认视图）
    //   月份固定两位：format!("{m:02}") 保证 "03" 而非 "3"
    let now_ym =
        sqlx::query_scalar::<_, String>("SELECT strftime('%Y-%m', date('now','localtime'))")
            .fetch_one(&state.pool)
            .await
            .map_err(AppError::Database)?;
    let target_year = query
        .year
        .clone()
        .unwrap_or_else(|| now_ym[..4].to_string());
    let target_month = query
        .month
        .clone()
        .unwrap_or_else(|| now_ym[5..7].to_string());
    let target_ym = format!("{target_year}-{target_month}");

    // 【M5 修订：按动作查看 —— 全部动作（id + 名字 + 部位）】
    let all_exercises = sqlx::query_as::<_, Exercise>("SELECT * FROM exercises WHERE user_id = ?")
        .bind(&user.id)
        .fetch_all(&state.pool)
        .await
        .map_err(AppError::Database)?;
    // 部位下拉选项（去重 + 排序，"全部"用空串）
    let mut part_list: Vec<String> = all_exercises
        .iter()
        .map(|ex| ex.body_part.clone())
        .collect::<std::collections::HashSet<String>>()
        .into_iter()
        .collect();
    part_list.sort();
    let ex_part_options = part_list
        .iter()
        .map(|p| format!(r#"<option value="{p}">{p}</option>"#, p = p))
        .collect::<Vec<String>>()
        .join("\n");
    // 动作列表：每个动作一个链接（data-part 供 JS 部位筛选）
    let ex_links = all_exercises
        .iter()
        .map(|ex| {
            format!(
                r#"<div class="ex-row" data-part="{part}"><a href="/exercises/{id}/stats">{name}</a></div>"#,
                part = ex.body_part,
                id = ex.id,
                name = ex.name,
            )
        })
        .collect::<Vec<String>>()
        .join("\n");

    // 【M5 修订：年份选项 —— 训练日年份去重 + 目标年兜底】
    //   跳历史年份时目标年可能不在训练日集合里 → 手动并入（insert 天然去重）
    let mut year_set: std::collections::HashSet<String> = non_empty_train_dts
        .iter()
        .map(|dt| dt[..4].to_string())
        .collect();
    year_set.insert(target_year.clone());
    let mut year_list: Vec<String> = year_set.into_iter().collect();
    year_list.sort_by(|a, b| b.cmp(a));
    let year_options = year_list
        .iter()
        .map(|y| {
            format!(
                r#"<option value="{y}"{sel}>{y}</option>"#,
                sel = if *y == target_year { " selected" } else { "" },
            )
        })
        .collect::<Vec<String>>()
        .join("\n");
    // 月份选项：01-12，selected 目标月
    let month_options = (1..=12)
        .map(|m| {
            let mm = format!("{m:02}");
            format!(
                r#"<option value="{mm}"{sel}>{mm}月</option>"#,
                sel = if mm == target_month { " selected" } else { "" },
            )
        })
        .collect::<Vec<String>>()
        .join("\n");

    // 【教学：目标月天数也让 SQLite 算——日期纪律】
    // 链式日期运算：目标月 1 号 → +1 月 → -1 天 = 目标月最后一天，
    // 再 strftime('%d') 取"日"，CAST 转整数 → 31/30/28/29
    let days_in_month = sqlx::query_scalar::<_, i64>(
        "SELECT CAST(strftime('%d', date(?, '+1 month', '-1 day')) AS INTEGER)",
    )
    .bind(format!("{target_ym}-01"))
    .fetch_one(&state.pool)
    .await
    .map_err(AppError::Database)?;

    // 日历单元格：1..=目标月天数，每 7 格换一行
    //   【M5 修订：颜色填充替代 ● 标记】
    //   - 有记录的日子 → 绿色背景 + 链接到 /history/{date}
    //   - 无记录 → 灰色背景（视觉上"空"更直观）
    // {day:02} = 零填充两位数（08-03 的"03"）
    let cells = (1..=days_in_month)
        .map(|day| {
            let date_str = format!("{target_ym}-{day:02}");
            let is_train_day = non_empty_train_dts.iter().any(|dt| dt == &date_str);
            let cell = if is_train_day
            {
                format!(
                    r#"<td style="background-color:#b7e4b0"><a href="/history/{date_str}">{day}</a></td>"#
                )
            }
            else
            {
                format!(r#"<td style="background-color:#dddddd">{day}</td>"#)
            };
            // 每 7 格换行；最后一天恰好整行时不多插空行
            if day % 7 == 0 && day != days_in_month
            {
                format!("{cell}</tr><tr>")
            }
            else
            {
                cell
            }
        })
        .collect::<String>();

    // 【M5 修订：按动作查看 —— 直接展示（部位下拉筛选）】
    //   原 checkbox 勾选是在"有训练日列表"前提下避免页面过长；
    //   训练日列表已移除，页面只剩日历 + 动作列表，直接展示即可。
    Ok(Html(format!(
        r#"<head><meta name="viewport" content="width=device-width, initial-scale=1.0"></head>
        <h2>历史回顾</h2>
        <p>年份：
        <select id="cal-year-filter" onchange="changeCalMonth()">
            {year_options}
        </select>
        月份：
        <select id="cal-month-filter" onchange="changeCalMonth()">
            {month_options}
        </select></p>
        <h3>日历</h3>
        <table border="1">
        <tr><th>一</th><th>二</th><th>三</th><th>四</th><th>五</th><th>六</th><th>日</th></tr>
        <tr>{cells}</tr>
        </table>
        <h3>按动作查看</h3>
        <p>部位：
        <select id="ex-part-filter" onchange="filterExByPart()">
            <option value="">全部</option>
            {ex_part_options}
        </select></p>
        <div id="ex-list-rows">{ex_links}</div>
        <p><a href="/">返回首页</a></p>
        <script>
            {javascript}
        </script>"#,
        cells = cells,
        ex_part_options = ex_part_options,
        ex_links = ex_links,
        year_options = year_options,
        month_options = month_options,
        javascript = r#"function filterExByPart(){
            var part = document.getElementById('ex-part-filter').value;
            document.querySelectorAll('#ex-list-rows .ex-row').forEach(function(row){
                row.style.display = (part === '' || row.getAttribute('data-part') === part) ? '' : 'none';
            });
        }
        function changeCalMonth(){
            var y = document.getElementById('cal-year-filter').value;
            var m = document.getElementById('cal-month-filter').value;
            if (y && m) { window.location.href = '/history?year=' + y + '&month=' + m; }
        }"#,
    )))
}

// ============================================================
// 第二部分：当天详情页（GET /history/{date}）
// ============================================================
/// 某天的全部训练记录（只读展示）
///
/// 【教学：Path<String> 和日期校验】
///   - 日期作为路径参数是字符串（'YYYY-MM-DD'），不是 i64
///   - 防御性校验：格式不对（不是 10 位、中间没有 '-'）→ 400/404
///     别让任意字符串进 SQL（虽然绑定参数防注入，但语义要干净）
///
/// 实现步骤：
/// 1. 签名：State + AuthUser + Path(date): Path<String>
/// 2. 校验日期格式（简单检查：长度 10、第 5/8 位是 '-'）
/// 3. 查该天全部记录：
///    ⚠️ records 表没有 user_id 列！数据隔离要走 JOIN：
///    SELECT r.* FROM records r
///    INNER JOIN exercises e ON r.exercise_id = e.id
///    WHERE e.user_id = ? AND r.record_date = ?
///    ORDER BY r.created_at
///    （records 只挂 plan_item_id/phase_id/exercise_id，
///    用户归属要经过 exercises 才能确定——M5 隔离纪律）
/// 4. 动作名：沿用 M4 模式——查全部动作 → HashMap<i64, String>
///    SELECT * FROM exercises WHERE user_id = ?
///    （为什么不用 JOIN？query_as 按列名匹配，JOIN 多出的列与
///     Record 结构体不匹配——M4.md 第 1 步讲过，M5 理解验证第 5 题）
/// 5. 每条记录渲染一行：动作名 | 重量 | 组×次 | 休息 | 1RM(Epley)
///    | 感受 | 策略 | 要领
///    1RM 调 calc::epley_1rm(record.weight, record.reps)
///    （无效记录 weight/reps <= 0 时公式返回 0，页面显示 "-"）
/// 6. 顶部返回历史首页链接；该天无记录 → 空态"这一天没有训练记录"
pub async fn history_day(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(date): Path<String>,
) -> Result<Html<String>, AppError>
{
    match date.split('-').collect::<Vec<&str>>().as_slice()
    {
        [yyyy, mm, dd] =>
        {
            yyyy.parse::<i64>()
                .map_err(|_| AppError::Validation("年份必须是数字".to_string()))?;
            mm.parse::<i64>()
                .map_err(|_| AppError::Validation("月份必须是数字".to_string()))?;
            dd.parse::<i64>()
                .map_err(|_| AppError::Validation("日必须是数字".to_string()))?;
        },
        _ =>
        {
            return Err(AppError::Validation(
                "日期格式必须是 YYYY-MM-DD".to_string(),
            ));
        },
    }

    let all_records_that_day = sqlx::query_as::<_, Record>(
        "SELECT r.* FROM records r
    INNER JOIN exercises e ON r.exercise_id = e.id
    WHERE e.user_id = ? AND r.record_date = ?
    ORDER BY e.sort_order ASC",
    )
    .bind(&user.id)
    .bind(&date)
    .fetch_all(&state.pool)
    .await
    .map_err(AppError::Database)?
    .into_iter()
    .map(|rec| {
        let rm = epley_1rm(rec.weight, rec.reps);
        (rec, rm)
    })
    .collect::<Vec<(Record, f64)>>();

    let all_exercises = sqlx::query_as::<_, Exercise>("SELECT * FROM exercises WHERE user_id = ?")
        .bind(&user.id)
        .fetch_all(&state.pool)
        .await
        .map_err(AppError::Database)?
        .into_iter()
        .map(|ex| (ex.id, ex.name))
        .collect::<HashMap<i64, String>>();

    // —— 以下为渲染部分（HTML 拼接，老师代写）——
    // 【教学：M4_bugfix_notes §6 约定——前端 DOM/HTML 部分 vibe coding 不补课。
    //   你写的后端逻辑（校验 + 查询 + 索引 + 1RM 预计算）到这里为止。】
    // 标题用校验解析出的 yyyy/mm/dd——既展示给用户，又"用上"了校验结果。

    // 空态：该天没有记录
    if all_records_that_day.is_empty()
    {
        return Ok(Html(format!(
            r#"<head><meta name="viewport" content="width=device-width, initial-scale=1.0"></head>
            <h2>{date} 训练记录</h2>
            <p>这一天没有训练记录</p>
            <p><a href="/history">返回历史回顾</a></p>"#
        )));
    }

    // 表格行：动作名（链到动作详情，下钻第 3 层）| 重量 | 组×次 | 休息
    //   | 1RM(Epley) | 感受 | 策略 | 要领
    //   1RM 无效（公式返回 0）→ 显示 "-"（calc.rs 的边界约定）
    let rows = all_records_that_day
        .iter()
        .map(|(rec, rm)| {
            let name = all_exercises
                .get(&rec.exercise_id)
                .map(|s| s.as_str())
                .unwrap_or("未知动作");
            let rm_text = if *rm <= 0.0
            {
                "-".to_string()
            }
            else
            {
                format!("{rm:.1}")
            };
            format!(
                r#"<tr>
                <td><a href="/exercises/{ex_id}/stats">{name}</a></td>
                <td>{weight}kg</td>
                <td>{sets}组*{reps}次</td>
                <td>{rest}秒</td>
                <td>{rm_text}</td>
                <td>{feeling}</td>
                <td>{strategy}</td>
                <td>{key_points}</td>
                </tr>"#,
                ex_id = rec.exercise_id,
                name = name,
                weight = rec.weight,
                sets = rec.sets,
                reps = rec.reps,
                rest = rec.rest,
                rm_text = rm_text,
                feeling = rec.feeling,
                strategy = rec.strategy,
                key_points = rec.key_points,
            )
        })
        .collect::<String>();

    Ok(Html(format!(
        r#"<head><meta name="viewport" content="width=device-width, initial-scale=1.0"></head>
        <h2>{date} 训练记录</h2>
        <table border="1">
        <tr><th>动作</th><th>重量</th><th>组*次</th><th>休息</th>
        <th>1RM(Epley)</th><th>感受</th><th>策略</th><th>要领</th></tr>
        {rows}
        </table>
        <p><a href="/history">返回历史回顾</a></p>"#
    )))
}

// ============================================================
// 第三部分：动作详情页（GET /exercises/{id}/stats）★ M5 重点
// ============================================================
/// 某动作全部历史：表格 + 重量/1RM 折线图（Chart.js）
///
/// 【教学：本页是 M5 的"渐进超负荷观察窗口"】
///   表格看单次记录细节；折线图看长期趋势：
///   - 重量线（weight）：每次实际举多重
///   - 1RM 线（Epley）：归一化后的进步趋势（不同次数可比）
///
/// 实现步骤：
/// 1. 签名：State + AuthUser + Path(id): Path<i64>
/// 2. 数据隔离纪律：先验证动作存在且属于当前用户
///    SELECT * FROM exercises WHERE id = ? AND user_id = ?
///    → 没有 → 404（先验证再查记录，404 语义才清晰）
/// 3. 查该动作全部记录（按日期升序，画折线图必须时间有序）：
///    SELECT * FROM records WHERE exercise_id = ?
///    ORDER BY record_date, id
///    （⚠️ 这里不需要 user_id 条件：第 2 步已验证动作归属，
///    exercise_id 已确定属于当前用户；records 表本身没有 user_id 列）
/// 4. 表格渲染：日期 | 重量 | 组×次 | 1RM(Epley) | 感受 | 策略
/// 5. 折线图数据：遍历记录生成两个数组
///    - 标签：record_date 列表
///    - 重量点：weight 列表
///    - 1RM 点：epley_1rm(weight, reps) 列表
/// 6. 数据注入（M4 §10.3 同款模式——数据单一来源）：
///    serde_json::to_string 序列化后注入页面 JS 变量：
///    let chart_json = serde_json::to_string(&points)?;
///    → format!(r#"var CHART_POINTS = {chart_json};"#)
///    Chart.js CDN：<script src="https://cdn.jsdelivr.net/npm/chart.js"></script>
///    然后 <script> 里用 CHART_POINTS 画两条线
///    （X 轴 label 用日期字符串，Chart.js 按数组顺序画即可）
/// 7. 记录数 < 2 → 不画图，提示"记录太少，攒几次训练再看趋势"
///    无记录 → 空态"这个动作还没有记录"
///
/// 【教学：为什么 JSON 注入而不是页面内嵌 JS 数组？】
///   Rust 端拼 JS 数组字面量容易出错（引号转义、数字格式），
///   serde_json 序列化是"机器生成的合法 JSON"，
///   前端直接 var CHART_POINTS = [...] 就是合法 JS——
///   这就是 M4_bugfix_notes §10.3 的"数据注入 vs 手写"。
pub async fn exercise_stats(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(id): Path<i64>,
) -> Result<Html<String>, AppError>
{
    // ① 数据隔离纪律：先验证动作存在且属于当前用户
    //    （同时拿到动作名，渲染页面标题用——查询结果别丢）
    let exercise =
        sqlx::query_as::<_, Exercise>("SELECT * FROM exercises WHERE id = ? AND user_id = ?")
            .bind(&id)
            .bind(&user.id)
            .fetch_optional(&state.pool)
            .await
            .map_err(AppError::Database)?
            .ok_or_else(|| AppError::NotFound("No such exercise in your profile".to_string()))?;
    let all_records = sqlx::query_as::<_, Record>(
        "SELECT * FROM records WHERE exercise_id = ? ORDER BY record_date ASC, id",
    )
    .bind(&id)
    .fetch_all(&state.pool)
    .await
    .map_err(AppError::Database)?
    .into_iter()
    .map(|rec| {
        let one_rm = epley_1rm(rec.weight, rec.reps);
        let two_rm = if rec.reps > 2 && wathan_mrm(one_rm, 2) > rec.weight
        {
            wathan_mrm(one_rm, 2)
        }
        else
        {
            rec.weight
        };
        let three_rm = if rec.reps > 3 && wathan_mrm(one_rm, 3) > rec.weight
        {
            wathan_mrm(one_rm, 3)
        }
        else
        {
            rec.weight
        };
        (rec, one_rm, two_rm, three_rm)
    })
    .collect::<Vec<(Record, f64, f64, f64)>>();

    // —— 以下为渲染部分（HTML/JS 拼接，老师代写）——
    // 【教学：M4_bugfix_notes §6 约定——前端 DOM/Chart.js 部分 vibe coding 不补课。
    //   你写的后端逻辑（归属验证 + 查询 + 1RM/2RM/3RM 计算）到这里为止。】

    // 空态：一条记录都没有
    if all_records.is_empty()
    {
        return Ok(Html(format!(
            r#"<head><meta name="viewport" content="width=device-width, initial-scale=1.0"></head>
            <h2>{name} 的历史记录</h2>
            <p>这个动作还没有记录</p>
            <p><a href="/history">返回历史回顾</a></p>"#,
            name = exercise.name,
        )));
    }

    // 表格行：日期 | 重量 | 组*次 | 1RM | 2RM | 3RM | 感受 | 策略
    //   （按日期升序展示，最新在最后，和折线图顺序一致）
    //   {v:.1} = 保留 1 位小数；无效记录（1RM=0）显示 "-"
    let rows = all_records
        .iter()
        .map(|(rec, one_rm, two_rm, three_rm)| {
            let fmt = |v: &f64| {
                if *v <= 0.0
                {
                    "-".to_string()
                }
                else
                {
                    format!("{v:.1}")
                }
            };
            format!(
                r#"<tr>
                <td>{date}</td>
                <td>{weight}kg</td>
                <td>{sets}组*{reps}次</td>
                <td>{one}</td>
                <td>{two}</td>
                <td>{three}</td>
                <td>{feeling}</td>
                <td>{strategy}</td>
                </tr>"#,
                date = rec.record_date,
                weight = rec.weight,
                sets = rec.sets,
                reps = rec.reps,
                one = fmt(one_rm),
                two = fmt(two_rm),
                three = fmt(three_rm),
                feeling = rec.feeling,
                strategy = rec.strategy,
            )
        })
        .collect::<String>();

    // 【M5 修订：图表抽取公共函数 exercise_chart_html（三个页面复用）】
    // 原内联的 labels/weights/one_rms/two_rms + serde_json 注入代码
    // 抽到文件底部的公共函数，时间范围改为最近 180 天。
    // None（记录 < 2 条）→ 显示提示文案。
    let chart_section = match exercise_chart_html(&state.pool, id).await?
    {
        Some(html) => html,
        None => "<p>记录太少，攒几次训练再看趋势</p>".to_string(),
    };

    Ok(Html(format!(
        r#"<head><meta name="viewport" content="width=device-width, initial-scale=1.0"></head>
        <h2>{name} 的历史记录</h2>
        <table border="1">
        <tr><th>日期</th><th>重量</th><th>组*次</th><th>1RM</th><th>2RM</th><th>3RM</th>
        <th>感受</th><th>策略</th></tr>
        {rows}
        </table>
        {chart_section}
        <p><a href="/history">返回历史回顾</a></p>"#,
        name = exercise.name,
    )))
}

// ============================================================
// 【M5 修订：公共图表函数 —— 三个页面复用】
// ============================================================
/// 生成某动作最近 180 天的"重量 / 1RM / 2RM"三折线图 HTML
/// （Chart.js CDN + serde_json 注入；records < 2 条 → 返回 None）
///
/// 【教学：为什么抽公共函数？】
/// 需求是三个页面显示同一张图：动作详情 / 记录表单 / 动作编辑表单。
/// 如果复制粘贴三份，改图表样式（颜色/线宽/时间范围）就要改三处，
/// 必然漂移（补课笔记 §10.3：数据/样式单一来源）。
/// 抽一个函数，三处调用——这就是"单一事实来源"的代码版。
///
/// 【教学：canvas id 唯一性】
/// 图表 JS 靠 getElementById 找 canvas，同一页面只有一个 canvas，
/// 固定 id 即可（本函数生成的图永远叫 trendChart）。
///
/// 时间范围：最近 180 天
///   WHERE record_date >= date('now', 'localtime', '-180 days')
///   边界含 180 天前当天（SQLite 日期运算，不引入 chrono）。
///
/// 返回：
///   - Some(html)：>= 2 条记录，返回完整图表 HTML
///   - None：< 2 条记录（或查询出错前），由调用方决定显示什么提示
pub async fn exercise_chart_html(
    pool: &SqlitePool,
    exercise_id: i64,
) -> Result<Option<String>, AppError>
{
    // 查最近 180 天记录（日期升序，折线图时间轴）
    let records = sqlx::query_as::<_, Record>(
        "SELECT * FROM records
        WHERE exercise_id = ?
        AND record_date >= date('now', 'localtime', '-180 days')
        ORDER BY record_date ASC, id",
    )
    .bind(exercise_id)
    .fetch_all(pool)
    .await
    .map_err(AppError::Database)?;

    // < 2 条 → 无趋势可画，返回 None
    if records.len() < 2
    {
        return Ok(None);
    }

    // 图表数据：三条线 + 日期标签（函数式提取，与 exercise_stats 旧逻辑一致）
    let labels: Vec<String> = records.iter().map(|rec| rec.record_date.clone()).collect();
    let weights: Vec<f64> = records.iter().map(|rec| rec.weight).collect();
    let one_rms: Vec<f64> = records
        .iter()
        .map(|rec| epley_1rm(rec.weight, rec.reps))
        .collect();
    let two_rms: Vec<f64> = records
        .iter()
        .map(|rec| {
            let one_rm = epley_1rm(rec.weight, rec.reps);
            // 与 exercise_stats 同款钳制：估算 2RM 低于实际重量 → 用实际值
            if rec.reps > 2 && wathan_mrm(one_rm, 2) > rec.weight
            {
                wathan_mrm(one_rm, 2)
            }
            else
            {
                rec.weight
            }
        })
        .collect();

    let chart_json = serde_json::to_string(&json!({
        "labels": labels,
        "weight": weights,
        "one_rm": one_rms,
        "two_rm": two_rms,
    }))
    .map_err(|e| AppError::Other(e.to_string()))?;

    Ok(Some(format!(
        r#"<div style="max-width:700px;margin:16px auto">
        <canvas id="trendChart"></canvas></div>
        <script src="https://cdn.jsdelivr.net/npm/chart.js"></script>
        <script>
        const CHART_POINTS = {chart_json};
        new Chart(document.getElementById('trendChart'), {{
            type: 'line',
            data: {{
                labels: CHART_POINTS.labels,
                datasets: [
                    {{ label: '重量(kg)', data: CHART_POINTS.weight,
                       borderColor: '#2196f3', tension: 0.2 }},
                    {{ label: '1RM(Epley)', data: CHART_POINTS.one_rm,
                       borderColor: '#e91e63', tension: 0.2 }},
                    {{ label: '2RM(Wathan)', data: CHART_POINTS.two_rm,
                       borderColor: '#4caf50', tension: 0.2 }}
                ]
            }},
            options: {{ responsive: true }}
        }});
        </script>"#
    )))
}

// ============================================================
// 【M5 修订：日历导航查询参数】
// ============================================================
/// GET /history?year=2026&month=08 —— 跳转查看任意年月的日历
///
/// 【教学：Query 提取器 + Option 字段（M2 exercises.rs 同款）】
/// 查询参数天然可选：不传 → None → 用默认（当前年月）。
#[derive(Deserialize)]
pub struct CalQuery
{
    pub year: Option<String>,
    pub month: Option<String>,
}
