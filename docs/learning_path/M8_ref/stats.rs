// ============================================================
// api/stats.rs —— 历史统计 API（M8 第 7 步）
// ============================================================
// 【教学说明】
// 历史统计只读不写（没有 INSERT/UPDATE，不需要事务）：
//
//   GET /api/v1/history                历史日历（?year=2026&month=03）
//   GET /api/v1/history/{date}         某天全部记录（含 1RM）
//   GET /api/v1/exercises/{id}/stats   某动作全部历史 + best_1rm
//
// 【教学：1RM 不落库 —— 实时计算】
// records 表没有 1RM 列：1RM = epley_1rm(weight, reps) 现算。
// 好处：改训练量（weight/reps）历史 1RM 自动变，不用回写历史。
// API 层复用 calc::epley_1rm（M5 第 1 步写的纯函数）。
//
// 📌 阶段要求：M8 你来实现本文件所有函数。
//   完整实现已备份在 docs/learning_path/M8_ref/，实现完成后对照检查。
// ============================================================
use axum::{
    Json,
    extract::{Path, Query, State},
};
use serde::{Deserialize, Serialize};

use crate::{
    AppState,
    api::{ApiError, auth::ApiAuthUser},
    calc::epley_1rm,
    models::{Exercise, Record},
};

// ============================================================
// 【教学：DTO —— 历史日历】
// ============================================================
// {
//   "year": "2026",
//   "month": "03",
//   "train_days": ["2026-03-01", "2026-03-15", ...]
// }
// 客户端自己按 train_days 画日历（iced 不需要渲染 HTML）。
#[derive(Serialize)]
pub struct CalendarOut
{
    pub year: String,
    pub month: String,
    /// 该月有训练记录的日期（YYYY-MM-DD 列表）
    pub train_days: Vec<String>,
}

// ============================================================
// 历史日历（GET /api/v1/history?year=2026&month=03）
// ============================================================
/// 历史日历：某月有记录的日期列表
///
/// 【教学：与页面层 calendar 的差异】
/// 页面层渲染 HTML 日历网格；API 层只返回数据（train_days），
/// 客户端自己画。缺省参数 → 当前年月。
///
/// 【实现步骤】
/// 1. 签名：State + ApiAuthUser + Query<CalendarQuery>
/// 2. 目标年月：query 传了就校验（4 位年 + 2 位月），没传用当前年月
///    SELECT strftime('%Y-%m', date('now','localtime'))
/// 3. 查该月有记录的日期：
///    SELECT DISTINCT record_date FROM records r
///    INNER JOIN exercises e ON r.exercise_id = e.id
///    WHERE e.user_id = ? AND record_date LIKE ? ORDER BY record_date
///    （LIKE '2026-03%' 前缀匹配）
/// 4. 组装 CalendarOut
#[derive(Deserialize)]
pub struct CalendarQuery
{
    pub year: Option<String>,
    pub month: Option<String>,
}

pub async fn calendar(
    State(state): State<AppState>,
    ApiAuthUser(user): ApiAuthUser,
    Query(query): Query<CalendarQuery>,
) -> Result<Json<CalendarOut>, ApiError>
{
    let pool = state.pool.read().await.clone();

    // 目标年月：参数优先，默认当前年月
    let now_ym =
        sqlx::query_scalar::<_, String>("SELECT strftime('%Y-%m', date('now','localtime'))")
            .fetch_one(&pool)
            .await
            .map_err(ApiError::Database)?;

    let year = query.year.unwrap_or_else(|| now_ym[..4].to_string());
    let month = query.month.unwrap_or_else(|| now_ym[5..7].to_string());

    // 校验：年份 4 位数字、月份 2 位数字
    if year.len() != 4 || !year.chars().all(|c| c.is_ascii_digit())
    {
        return Err(ApiError::Validation("年份格式错误".to_string()));
    }
    if month.len() != 2 || !month.chars().all(|c| c.is_ascii_digit())
    {
        return Err(ApiError::Validation("月份格式错误".to_string()));
    }

    let prefix = format!("{year}-{month}");

    let train_days = sqlx::query_scalar::<_, String>(
        "SELECT DISTINCT record_date FROM records r
        INNER JOIN exercises e ON r.exercise_id = e.id
        WHERE e.user_id = ? AND record_date LIKE ?
        ORDER BY record_date",
    )
    .bind(&user.id)
    .bind(format!("{prefix}%"))
    .fetch_all(&pool)
    .await
    .map_err(ApiError::Database)?;

    Ok(Json(CalendarOut {
        year,
        month,
        train_days,
    }))
}

// ============================================================
// 某天详情（GET /api/v1/history/{date}）
// ============================================================
/// 某天全部训练记录（含 1RM）
///
/// 【教学：数据隔离 —— records 表没有 user_id，走 JOIN exercises】
/// 页面层同款 SQL（M5 注释里的"数据隔离纪律"）：
///   SELECT ... FROM records r
///   INNER JOIN exercises e ON r.exercise_id = e.id
///   WHERE e.user_id = ?
/// records 只挂 exercise_id/plan_item_id，用户归属经 exercises 确定。
///
/// 【教学：1RM 字段名 —— "1rm" 不是合法 Rust 标识符】
/// JSON 键用 serde rename："1rm"（全小写，和 M8.md 一致）
/// 或 epley 1rm 计算。页面层显示 "1RM(Epley)"，API 键名简化。
///
/// 【实现步骤】
/// 1. 签名：State + ApiAuthUser + Path(date)
/// 2. 校验日期格式（YYYY-MM-DD）
/// 3. 查该天全部记录（JOIN exercises 数据隔离）
///    → 元组查询（和页面层同款，拿部位/模式/杆重）
/// 4. 查动作索引（id → 名字）
/// 5. 组装 DayRecordOut（含 1rm）
#[derive(Serialize)]
pub struct DayRecordOut
{
    pub id: i64,
    pub exercise_id: i64,
    pub exercise_name: String,
    pub body_part: String,
    pub mode: String,
    pub weight: f64,
    pub sets: i64,
    pub reps: i64,
    pub rest: i64,
    pub feeling: String,
    pub strategy: String,
    pub key_points: String,
    /// Epley 1RM（实时计算，无效记录 → 0.0）
    #[serde(rename = "1rm")]
    pub one_rm: f64,
}

pub async fn history_day(
    State(state): State<AppState>,
    ApiAuthUser(user): ApiAuthUser,
    Path(date): Path<String>,
) -> Result<Json<Vec<DayRecordOut>>, ApiError>
{
    let pool = state.pool.read().await.clone();

    // 日期格式校验
    validate_date(&date)?;

    // 查该天记录（JOIN exercises 数据隔离）
    // 元组：(id, exercise_id, mode, weight, sets, reps, rest,
    //        feeling, strategy, key_points, bar, unit, body_part)
    let rows_raw = sqlx::query_as::<
        _,
        (
            i64,
            i64,
            String,
            f64,
            i64,
            i64,
            i64,
            String,
            String,
            String,
            Option<f64>,
            String,
            String,
        ),
    >(
        "SELECT r.id, r.exercise_id, r.mode, r.weight, r.sets, r.reps, r.rest,
        r.feeling, r.strategy, r.key_points,
        COALESCE(pi.plan_bar_weight, e.bar_weight) AS bar,
        e.default_unit, e.body_part
        FROM records r
        INNER JOIN exercises e ON r.exercise_id = e.id
        LEFT JOIN plan_items pi ON r.plan_item_id = pi.id
        WHERE e.user_id = ? AND r.record_date = ?
        ORDER BY e.sort_order ASC, r.id",
    )
    .bind(&user.id)
    .bind(&date)
    .fetch_all(&pool)
    .await
    .map_err(ApiError::Database)?;

    // 动作名索引（id → 名字）
    let ex_names: std::collections::HashMap<i64, String> =
        sqlx::query_as::<_, Exercise>("SELECT * FROM exercises WHERE user_id = ?")
            .bind(&user.id)
            .fetch_all(&pool)
            .await
            .map_err(ApiError::Database)?
            .into_iter()
            .map(|e| (e.id, e.name))
            .collect();

    // 组装（1RM 实时算）
    let out = rows_raw
        .iter()
        .map(
            |(
                id,
                ex_id,
                mode,
                weight,
                sets,
                reps,
                rest,
                feeling,
                strategy,
                key_points,
                _bar,
                _unit,
                body_part,
            )| {
                DayRecordOut {
                    id: *id,
                    exercise_id: *ex_id,
                    exercise_name: ex_names
                        .get(ex_id)
                        .cloned()
                        .unwrap_or_else(|| "未知动作".to_string()),
                    body_part: body_part.clone(),
                    mode: mode.clone(),
                    weight: *weight,
                    sets: *sets,
                    reps: *reps,
                    rest: *rest,
                    feeling: feeling.clone(),
                    strategy: strategy.clone(),
                    key_points: key_points.clone(),
                    one_rm: epley_1rm(*weight, *reps),
                }
            },
        )
        .collect();

    Ok(Json(out))
}

// ============================================================
// 某动作统计（GET /api/v1/exercises/{id}/stats）
// ============================================================
/// 某动作全部历史 + best_1rm
///
/// 【教学：与页面层 exercise_stats 的差异】
/// 页面层渲染表格 + Chart.js 折线图；API 层只返回数据：
/// {
///   "exercise": {"id": 1, "name": "深蹲"},
///   "records": [{"date": "...", "weight": 100, "sets": 5, "reps": 5, "1rm": 113.6}, ...],
///   "best_1rm": 120.5
/// }
/// 客户端自己画趋势图（iced 用 plotters 或简单折线）。
///
/// 【实现步骤】
/// 1. 签名：State + ApiAuthUser + Path(id)
/// 2. 归属验证：SELECT * FROM exercises WHERE id = ? AND user_id = ?
/// 3. 查全部记录：SELECT * FROM records WHERE exercise_id = ?
///    ORDER BY record_date ASC, id ASC
/// 4. 组装 ExerciseStatsOut（1rm 实时算，best_1rm = max）
#[derive(Serialize)]
pub struct ExerciseStatsOut
{
    pub exercise: ExerciseBriefOut,
    pub records: Vec<ExerciseRecordOut>,
    pub best_1rm: f64,
}

#[derive(Serialize)]
pub struct ExerciseBriefOut
{
    pub id: i64,
    pub name: String,
    pub body_part: String,
}

#[derive(Serialize)]
pub struct ExerciseRecordOut
{
    pub date: String,
    pub weight: f64,
    pub sets: i64,
    pub reps: i64,
    #[serde(rename = "1rm")]
    pub one_rm: f64,
}

pub async fn exercise_stats(
    State(state): State<AppState>,
    ApiAuthUser(user): ApiAuthUser,
    Path(id): Path<i64>,
) -> Result<Json<ExerciseStatsOut>, ApiError>
{
    let pool = state.pool.read().await.clone();

    // 归属验证
    let ex = sqlx::query_as::<_, Exercise>("SELECT * FROM exercises WHERE id = ? AND user_id = ?")
        .bind(&id)
        .bind(&user.id)
        .fetch_optional(&pool)
        .await
        .map_err(ApiError::Database)?
        .ok_or_else(|| ApiError::NotFound("动作不存在".to_string()))?;

    // 全部记录（升序 → 趋势自然有序）
    let records = sqlx::query_as::<_, Record>(
        "SELECT * FROM records WHERE exercise_id = ? ORDER BY record_date ASC, id ASC",
    )
    .bind(&id)
    .fetch_all(&pool)
    .await
    .map_err(ApiError::Database)?;

    // 组装（1rm 实时算）
    let recs_out = records
        .iter()
        .map(|r| ExerciseRecordOut {
            date: r.record_date.clone(),
            weight: r.weight,
            sets: r.sets,
            reps: r.reps,
            one_rm: epley_1rm(r.weight, r.reps),
        })
        .collect::<Vec<_>>();

    // best_1rm：全记录 1RM 最大值（epley 无效输入返回 0.0，正好忽略）
    let best_1rm = recs_out.iter().map(|r| r.one_rm).fold(0.0f64, f64::max);

    Ok(Json(ExerciseStatsOut {
        exercise: ExerciseBriefOut {
            id: ex.id,
            name: ex.name.clone(),
            body_part: ex.body_part.clone(),
        },
        records: recs_out,
        best_1rm,
    }))
}

// ============================================================
// 【教学：日期格式校验 —— YYYY-MM-DD】
// ============================================================
fn validate_date(date: &str) -> Result<(), ApiError>
{
    match date.split('-').collect::<Vec<&str>>().as_slice()
    {
        [yyyy, mm, dd] =>
        {
            yyyy.parse::<i64>()
                .map_err(|_| ApiError::Validation("年份必须是数字".to_string()))?;
            mm.parse::<i64>()
                .map_err(|_| ApiError::Validation("月份必须是数字".to_string()))?;
            dd.parse::<i64>()
                .map_err(|_| ApiError::Validation("日必须是数字".to_string()))?;
            Ok(())
        },
        _ => Err(ApiError::Validation(
            "日期格式必须是 YYYY-MM-DD".to_string(),
        )),
    }
}
