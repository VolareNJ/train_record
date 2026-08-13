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
    extract::{Path, State},
    response::Html,
};
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
/// 历史首页：当月日历（有记录的日子有标记）+ 全部训练日列表
///
/// 【教学：页面结构】
///   上半部分：当月月历（7 列网格），有记录的日子显示可点击链接
///   下半部分：全部训练日列表（倒序），每个日期 → /history/{date}
///
/// 实现步骤：
/// 1. 签名：State + AuthUser
/// 2. 查当前用户全部非空训练日（倒序，最新在前）：
///    SELECT DISTINCT record_date FROM records
///    WHERE user_id = ? ORDER BY record_date DESC
///    （DISTINCT：同一天练多个动作会产生多行记录，去重后每天一行）
/// 3. 查"今天所在月"有记录的日期集合（用于日历标记）：
///    - 先拿当月前缀：SELECT strftime('%Y-%m', date('now','localtime'))
///      （如 '2026-08'）
///    - 再查：SELECT DISTINCT record_date FROM records
///      WHERE user_id = ? AND record_date LIKE '2026-08%'
///      （日期是零填充字符串 '2026-08-03'，LIKE '2026-08%' 能精确匹配当月）
/// 4. 渲染日历：7 列表格（周一~周日），
///    把当月 1~31 号排进格子，有记录的日期显示 <a> 链接
///    （简化：第一天固定从第一格开始，不必按真实星期对齐；
///     想对齐的话用 strftime('%w', ...) 拿星期偏移——选做）
/// 5. 渲染训练日列表：遍历第 2 步的结果，每天一个链接
/// 6. 空态：一条记录都没有 → 提示"还没有训练记录，去今日页开始第一次训练吧"
pub async fn history(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
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

    let current_month =
        sqlx::query_scalar::<_, String>("SELECT strftime('%Y-%m', date('now','localtime'))")
            .fetch_one(&state.pool)
            .await
            .map_err(AppError::Database)?;

    let current_month_train_dts = non_empty_train_dts
        .iter()
        .filter(|dt| dt.starts_with(&current_month))
        .collect::<Vec<&String>>();

    // —— 以下为渲染部分（HTML 拼接）——
    // 【教学：M4_bugfix_notes §6 约定——前端 DOM/HTML 部分 vibe coding 不补课，
    //   所以这半段老师代写。你写的后端逻辑到上面为止都是对的。】
    // 但注意：渲染前还有最后一点"后端逻辑"——当月天数。

    // 空态：一条记录都没有 → 引导去今日页
    if non_empty_train_dts.is_empty()
    {
        return Ok(Html(
            r#"<h2>历史回顾</h2>
            <p>还没有训练记录，去<a href="/today">今日页</a>开始第一次训练吧</p>
            <p><a href="/">返回首页</a></p>"#
                .to_string(),
        ));
    }

    // 【教学：当月天数也让 SQLite 算——日期纪律】
    // 链式日期运算：本月 1 号 → +1 月 → -1 天 = 当月最后一天，
    // 再 strftime('%d') 取"日"，CAST 转整数 → 31/30/28/29
    // 不引入 chrono、不让 Rust 手算闰年（M5.md 常见坑第 1 条）
    let days_in_month: i64 = sqlx::query_scalar(
        "SELECT CAST(strftime('%d', date('now', 'localtime',
        'start of month', '+1 month', '-1 day')) AS INTEGER)",
    )
    .fetch_one(&state.pool)
    .await
    .map_err(AppError::Database)?;

    // 日历单元格：1..=当月天数，每 7 格换一行
    //   - 有记录的日子 → ● 标记 + 链接到 /history/{date}
    //   - 无记录 → 纯文本日号
    // 简化：第一天固定从第一格开始，不做真实星期对齐（M5.md 已说明）
    // {day:02} = 零填充两位数（08-03 的"03"）
    let cells: String = (1..=days_in_month)
        .map(|day| {
            let date_str = format!("{}-{day:02}", current_month);
            let is_train_day = current_month_train_dts
                .iter()
                .any(|dt| dt.as_str() == date_str);
            let cell = if is_train_day
            {
                format!(r#"<td><a href="/history/{date_str}">●{day}</a></td>"#)
            }
            else
            {
                format!("<td>{day}</td>")
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
        .collect();

    // 训练日列表：全部记录日倒序，每天一个链接
    let date_links: String = non_empty_train_dts
        .iter()
        .map(|dt| format!(r#"<li><a href="/history/{dt}">{dt}</a></li>"#))
        .collect();

    Ok(Html(format!(
        r#"<h2>历史回顾</h2>
        <h3>{current_month} 日历（● = 有记录）</h3>
        <table border="1">
        <tr><th>一</th><th>二</th><th>三</th><th>四</th><th>五</th><th>六</th><th>日</th></tr>
        <tr>{cells}</tr>
        </table>
        <h3>全部训练日</h3>
        <ul>{date_links}</ul>
        <p><a href="/">返回首页</a></p>"#
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
    todo!("M5 第 3 步：实现当天详情页（全部记录 + 1RM）")
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
    todo!("M5 第 4 步：实现动作详情页（历史表格 + Chart.js 折线图）")
}
