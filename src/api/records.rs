// ============================================================
// api/records.rs —— 训练记录 API（M8 第 6 步）
// ============================================================
// 【教学说明】
// 训练记录是"计划项 → 记录"的关系（一个计划项当天一条记录）：
//
//   GET   /api/v1/today                    今日训练卡片（阶段/计划/动作项/最近记录）
//   POST  /api/v1/plans/{plan_id}/items/{item_id}/records   记录 upsert（当天有→更新，无→插入）
//   GET   /api/v1/records?date=YYYY-MM-DD  按日期查记录（全部动作）
//   PATCH /api/v1/records/{id}             更新某条记录
//   DELETE /api/v1/records/{id}            删除某条记录
//
// 【教学：upsert —— 一天一条】
// 一个计划项一天最多一条记录：先查该计划项最近一条记录，
// 有 → UPDATE（改同一行），无 → INSERT 新行。
// （页面 record_save 同款逻辑，但 API 简化：不做要领回写动作库）
//
// 📌 阶段要求：M8 你来实现本文件所有函数。
//   完整实现已备份在 docs/learning_path/M8_ref/，实现完成后对照检查。
// ============================================================
use axum::{
    Json,
    extract::{Path, Query, State},
};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

use crate::{
    AppState,
    api::{ApiError, auth::ApiAuthUser},
    models::{Exercise, Phase, Plan, PlanItem, Record},
};

// ============================================================
// 【教学：DTO —— 今日卡片】
// ============================================================
// 按 M8.md §2.3 定的 JSON 结构：
// {
//   "phase": {"id": 1, "name": "增肌", "days": 5},
//   "date": "2026-03-15",
//   "plan": {"id": 3, "note": "推日", "items": [...]}
// }
// 无进行中阶段 / 无今日计划 → phase / plan 为 null（iced 友好空态）
#[derive(Serialize)]
pub struct TodayItemOut
{
    pub id: i64,
    pub exercise_id: i64,
    pub exercise_name: String,
    pub body_part: String,
    pub plan_sets: Option<i64>,
    pub plan_reps: Option<i64>,
    pub plan_weight: Option<f64>,
    pub plan_rest: Option<i64>,
    pub plan_key_points: Option<String>,
    /// 该计划项最近一条记录（无 → None）
    pub last_record: Option<LastRecordOut>,
}

#[derive(Serialize)]
pub struct LastRecordOut
{
    pub id: i64,
    pub weight: f64,
    pub sets: i64,
    pub reps: i64,
    pub rest: i64,
    pub feeling: String,
    pub strategy: String,
}

#[derive(Serialize)]
pub struct TodayPlanOut
{
    pub id: i64,
    pub note: String,
    pub items: Vec<TodayItemOut>,
}

#[derive(Serialize)]
pub struct TodayPhaseOut
{
    pub id: i64,
    pub name: String,
    pub days: i64,
}

#[derive(Serialize)]
pub struct TodayOut
{
    pub phase: Option<TodayPhaseOut>,
    pub date: String,
    pub plan: Option<TodayPlanOut>,
}

// ============================================================
// 今日训练卡片（GET /api/v1/today）
// ============================================================
/// 今日训练卡片
///
/// 【教学：与页面 today 的差异】
/// 页面层空态返回"提示 HTML 页"；API 层空态返回
/// phase: null / plan: null（客户端自己决定 UI）。
/// 坚持天数（days）：start_date 为空 → 0。
///
/// 【实现步骤】
/// 1. 查进行中阶段：SELECT * FROM phases WHERE user_id = ? AND archived = 0
///    ORDER BY created_at DESC LIMIT 1 → 无 → phase: null
/// 2. 查今天：SELECT date('now', 'localtime')
/// 3. 查今日计划：SELECT * FROM plans WHERE phase_id = ? AND date = ?
///    → 无 → plan: null
/// 4. 查计划项 + 动作索引（HashMap id → (name, body_part)）
/// 5. 每个计划项查最近记录（ORDER BY record_date DESC, id DESC LIMIT 1）
/// 6. 组装 TodayOut
pub async fn today(
    State(state): State<AppState>,
    ApiAuthUser(user): ApiAuthUser,
) -> Result<Json<TodayOut>, ApiError>
{
    let pool = state.pool.read().await.clone();

    // 1. 进行中阶段
    let current_phase = sqlx::query_as::<_, Phase>(
        "SELECT * FROM phases WHERE user_id = ? AND archived = 0 ORDER BY created_at DESC LIMIT 1",
    )
    .bind(&user.id)
    .fetch_optional(&pool)
    .await
    .map_err(ApiError::Database)?;

    // 2. 今天
    let today_dt = sqlx::query_scalar::<_, String>("SELECT date('now', 'localtime')")
        .fetch_one(&pool)
        .await
        .map_err(ApiError::Database)?;

    // 3. 今日计划
    let today_plan = match &current_phase
    {
        Some(phase) =>
        {
            sqlx::query_as::<_, Plan>("SELECT * FROM plans WHERE phase_id = ? AND date = ?")
                .bind(&phase.id)
                .bind(&today_dt)
                .fetch_optional(&pool)
                .await
                .map_err(ApiError::Database)?
        },
        None => None,
    };

    // 4-6. 组装（有阶段且有计划才查 items）
    let phase_out = match &current_phase
    {
        None => None,
        Some(phase) =>
        {
            // 坚持天数（start_date 为空 → 0）
            let days = match &phase.start_date
            {
                Some(start_date) => sqlx::query_scalar::<_, i64>(
                    "SELECT CAST(julianday('now','localtime') - julianday(?) AS INTEGER)",
                )
                .bind(start_date)
                .fetch_one(&pool)
                .await
                .map_err(ApiError::Database)?,
                None => 0,
            };
            Some(TodayPhaseOut {
                id: phase.id,
                name: phase.name.clone(),
                days,
            })
        },
    };

    let plan_out = match &today_plan
    {
        None => None,
        Some(plan) =>
        {
            // 计划项
            let plan_items = sqlx::query_as::<_, PlanItem>(
                "SELECT * FROM plan_items WHERE plan_id = ? ORDER BY sort_order ASC",
            )
            .bind(&plan.id)
            .fetch_all(&pool)
            .await
            .map_err(ApiError::Database)?;

            // 动作索引（id → (name, body_part)）
            let ex_index: std::collections::HashMap<i64, (String, String)> =
                sqlx::query_as::<_, Exercise>("SELECT * FROM exercises WHERE user_id = ?")
                    .bind(&user.id)
                    .fetch_all(&pool)
                    .await
                    .map_err(ApiError::Database)?
                    .into_iter()
                    .map(|e| (e.id, (e.name, e.body_part)))
                    .collect();

            // 每个计划项的最近记录
            let mut items = Vec::with_capacity(plan_items.len());
            for item in &plan_items
            {
                let last = sqlx::query_as::<_, Record>(
                    "SELECT * FROM records WHERE plan_item_id = ?
                 ORDER BY record_date DESC, id DESC LIMIT 1",
                )
                .bind(&item.id)
                .fetch_optional(&pool)
                .await
                .map_err(ApiError::Database)?;

                let (ex_name, body_part) = ex_index
                    .get(&item.exercise_id)
                    .cloned()
                    .unwrap_or_else(|| ("未知动作".to_string(), "未分组".to_string()));

                items.push(TodayItemOut {
                    id: item.id,
                    exercise_id: item.exercise_id,
                    exercise_name: ex_name,
                    body_part,
                    plan_sets: item.plan_sets,
                    plan_reps: item.plan_reps,
                    plan_weight: item.plan_weight,
                    plan_rest: item.plan_rest,
                    plan_key_points: item.plan_key_points.clone(),
                    last_record: last.map(|r| LastRecordOut {
                        id: r.id,
                        weight: r.weight,
                        sets: r.sets,
                        reps: r.reps,
                        rest: r.rest,
                        feeling: r.feeling,
                        strategy: r.strategy,
                    }),
                });
            }

            Some(TodayPlanOut {
                id: plan.id,
                note: plan.note.clone(),
                items,
            })
        },
    };

    Ok(Json(TodayOut {
        phase: phase_out,
        date: today_dt,
        plan: plan_out,
    }))
}

// ============================================================
// 【教学：RecordCreateReq —— 记录请求体】
// ============================================================
// 数字字段直接 f64/i64（JSON 类型安全，无页面层 String parse 的坑）。
// completed 默认 false（不传就是未完成）。
#[derive(Deserialize)]
pub struct RecordCreateReq
{
    pub weight: f64,
    pub sets: i64,
    pub reps: i64,
    #[serde(default)]
    pub rest: i64,
    #[serde(default)]
    pub feeling: String,
    #[serde(default)]
    pub strategy: String,
    #[serde(default)]
    pub key_points: String,
    #[serde(default)]
    pub completed: bool,
}

// ============================================================
// 【教学：RecordOut —— 记录 DTO】
// ============================================================
// 附动作名（JOIN exercises 或查动作索引）
#[derive(Serialize)]
pub struct RecordOut
{
    pub id: i64,
    pub exercise_id: i64,
    pub exercise_name: String,
    pub record_date: String,
    pub weight: f64,
    pub sets: i64,
    pub reps: i64,
    pub rest: i64,
    pub feeling: String,
    pub strategy: String,
    pub key_points: String,
    pub mode: String,
    pub completed: bool,
}

// ============================================================
// 记录 upsert（POST /api/v1/plans/{plan_id}/items/{item_id}/records）
// ============================================================
/// 记录 upsert：当天有 → 更新，无 → 插入
///
/// 【教学：验证链路（数据隔离）】
/// 1. 验证计划归属：SELECT p.* FROM plans p INNER JOIN phases ph
///    ON p.phase_id = ph.id WHERE p.id = ? AND ph.user_id = ?
/// 2. 验证阶段未归档（归档阶段不可编辑）
/// 3. 验证计划项属于该计划：WHERE id = ? AND plan_id = ?（双条件防越权）
/// 4. 负数校验（训练数据不可能是负数）
/// 5. 查该计划项最近记录：有 → UPDATE，无 → INSERT
///
/// 【实现步骤】
/// 1. 签名：State + ApiAuthUser + Path((plan_id, item_id)) + Json(req)
/// 2. 三步验证（如上）
/// 3. 校验负数
/// 4. 查最近记录 → match：Some → UPDATE；None → INSERT（record_date = 今天）
/// 5. 返回保存后的记录 JSON（RecordOut）
pub async fn upsert_record(
    State(state): State<AppState>,
    ApiAuthUser(user): ApiAuthUser,
    Path((plan_id, item_id)): Path<(i64, i64)>,
    Json(req): Json<RecordCreateReq>,
) -> Result<Json<RecordOut>, ApiError>
{
    let pool = state.pool.read().await.clone();

    // 1. 计划归属
    let plan = sqlx::query_as::<_, Plan>(
        "SELECT p.* FROM plans p
        INNER JOIN phases ph ON p.phase_id = ph.id
        WHERE p.id = ? AND ph.user_id = ?",
    )
    .bind(&plan_id)
    .bind(&user.id)
    .fetch_optional(&pool)
    .await
    .map_err(ApiError::Database)?
    .ok_or_else(|| ApiError::NotFound("计划不存在".to_string()))?;

    // 2. 阶段未归档
    let phase = sqlx::query_as::<_, Phase>("SELECT * FROM phases WHERE id = ? AND user_id = ?")
        .bind(&plan.phase_id)
        .bind(&user.id)
        .fetch_optional(&pool)
        .await
        .map_err(ApiError::Database)?
        .ok_or_else(|| ApiError::NotFound("阶段不存在".to_string()))?;
    if phase.archived
    {
        return Err(ApiError::Forbidden("归档阶段不可编辑".to_string()));
    }

    // 3. 计划项属于该计划（双条件）+ 拿 exercise_id
    let plan_item =
        sqlx::query_as::<_, PlanItem>("SELECT * FROM plan_items WHERE id = ? AND plan_id = ?")
            .bind(&item_id)
            .bind(&plan_id)
            .fetch_optional(&pool)
            .await
            .map_err(ApiError::Database)?
            .ok_or_else(|| ApiError::NotFound("计划项不存在".to_string()))?;

    // 4. 负数校验
    if req.weight < 0.0 || req.sets < 0 || req.reps < 0 || req.rest < 0
    {
        return Err(ApiError::Validation(
            "重量/组数/次数/休息不能为负数".to_string(),
        ));
    }

    // 5. 查该计划项最近记录
    let most_recent = sqlx::query_as::<_, Record>(
        "SELECT * FROM records WHERE plan_item_id = ?
        ORDER BY record_date DESC, id DESC LIMIT 1",
    )
    .bind(&item_id)
    .fetch_optional(&pool)
    .await
    .map_err(ApiError::Database)?;

    // 6. 计重方式（mode）：API 简化 —— 用动作库默认值，不落库回写
    let mode = sqlx::query_scalar::<_, String>(
        "SELECT default_mode FROM exercises WHERE id = ? AND user_id = ?",
    )
    .bind(&plan_item.exercise_id)
    .bind(&user.id)
    .fetch_optional(&pool)
    .await
    .map_err(ApiError::Database)?
    .unwrap_or_else(|| "bar".to_string());

    // 7. INSERT 或 UPDATE（当天已有记录 → 更新同一行）
    let saved: Record = match most_recent
    {
        Some(record) => sqlx::query_as::<_, Record>(
            "UPDATE records SET completed = ?, weight = ?, sets = ?, reps = ?, rest = ?,
                feeling = ?, strategy = ?, key_points = ?
                WHERE id = ?
                RETURNING *",
        )
        .bind(&req.completed)
        .bind(&req.weight)
        .bind(&req.sets)
        .bind(&req.reps)
        .bind(&req.rest)
        .bind(&req.feeling)
        .bind(&req.strategy)
        .bind(&req.key_points)
        .bind(&record.id)
        .fetch_one(&pool)
        .await
        .map_err(ApiError::Database)?,
        None =>
        {
            let today_dt = sqlx::query_scalar::<_, String>("SELECT date('now', 'localtime')")
                .fetch_one(&pool)
                .await
                .map_err(ApiError::Database)?;
            sqlx::query_as::<_, Record>(
                "INSERT INTO records
                (plan_item_id, phase_id, exercise_id, record_date, completed,
                weight, sets, reps, rest, feeling, strategy, key_points, mode)
                VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                RETURNING *",
            )
            .bind(&plan_item.id)
            .bind(&phase.id)
            .bind(&plan_item.exercise_id)
            .bind(&today_dt)
            .bind(&req.completed)
            .bind(&req.weight)
            .bind(&req.sets)
            .bind(&req.reps)
            .bind(&req.rest)
            .bind(&req.feeling)
            .bind(&req.strategy)
            .bind(&req.key_points)
            .bind(&mode)
            .fetch_one(&pool)
            .await
            .map_err(ApiError::Database)?
        },
    };

    // 7. 查动作名（record_out）
    Ok(Json(record_out(&pool, &saved, user.id).await?))
}

// ============================================================
// 【教学：Record → RecordOut（补动作名）】
// ============================================================
async fn record_out(pool: &SqlitePool, r: &Record, user_id: i64) -> Result<RecordOut, ApiError>
{
    let ex_name =
        sqlx::query_scalar::<_, String>("SELECT name FROM exercises WHERE id = ? AND user_id = ?")
            .bind(&r.exercise_id)
            .bind(&user_id)
            .fetch_optional(pool)
            .await
            .map_err(ApiError::Database)?
            .unwrap_or_else(|| "未知动作".to_string());

    Ok(RecordOut {
        id: r.id,
        exercise_id: r.exercise_id,
        exercise_name: ex_name,
        record_date: r.record_date.clone(),
        weight: r.weight,
        sets: r.sets,
        reps: r.reps,
        rest: r.rest,
        feeling: r.feeling.clone(),
        strategy: r.strategy.clone(),
        key_points: r.key_points.clone(),
        mode: r.mode.clone(),
        completed: r.completed,
    })
}

// ============================================================
// 按日期查记录（GET /api/v1/records?date=YYYY-MM-DD）
// ============================================================
/// 按日期查记录（该用户当天所有动作记录，按动作名排序）
///
/// 【教学：日期格式校验】
/// date 必填，必须 YYYY-MM-DD（校验同 stats::history_day）。
#[derive(Deserialize)]
pub struct RecordListQuery
{
    pub date: String,
}

pub async fn list_by_date(
    State(state): State<AppState>,
    ApiAuthUser(user): ApiAuthUser,
    Query(query): Query<RecordListQuery>,
) -> Result<Json<Vec<RecordOut>>, ApiError>
{
    let pool = state.pool.read().await.clone();

    validate_date(&query.date)?;

    // ⚠️ records 表没有 user_id 列！数据隔离走 JOIN exercises（M5 纪律）：
    //   SELECT r.* FROM records r
    //   INNER JOIN exercises e ON r.exercise_id = e.id
    //   WHERE e.user_id = ? AND r.record_date = ?
    let records = sqlx::query_as::<_, Record>(
        "SELECT r.* FROM records r
        INNER JOIN exercises e ON r.exercise_id = e.id
        WHERE e.user_id = ? AND r.record_date = ?
        ORDER BY r.exercise_id",
    )
    .bind(&user.id)
    .bind(&query.date)
    .fetch_all(&pool)
    .await
    .map_err(ApiError::Database)?;

    let mut out = Vec::with_capacity(records.len());
    for r in &records
    {
        out.push(record_out(&pool, r, user.id).await?);
    }

    Ok(Json(out))
}

// ============================================================
// 更新记录（PATCH /api/v1/records/{id}）
// ============================================================
/// 更新某条记录（部分更新，先查旧值合并）
///
/// 【实现步骤】
/// 1. 验证记录归属：WHERE id = ? AND user_id = ?
/// 2. 合并新旧值（Option unwrap_or 旧值）
/// 3. 负数校验
/// 4. UPDATE ... RETURNING * → record_out
#[derive(Deserialize)]
pub struct RecordUpdateReq
{
    #[serde(default)]
    pub weight: Option<f64>,
    #[serde(default)]
    pub sets: Option<i64>,
    #[serde(default)]
    pub reps: Option<i64>,
    #[serde(default)]
    pub rest: Option<i64>,
    #[serde(default)]
    pub feeling: Option<String>,
    #[serde(default)]
    pub strategy: Option<String>,
    #[serde(default)]
    pub key_points: Option<String>,
    #[serde(default)]
    pub completed: Option<bool>,
}

pub async fn update_record(
    State(state): State<AppState>,
    ApiAuthUser(user): ApiAuthUser,
    Path(id): Path<i64>,
    Json(req): Json<RecordUpdateReq>,
) -> Result<Json<RecordOut>, ApiError>
{
    let pool = state.pool.read().await.clone();

    // 归属 + 取旧值（records 无 user_id 列 → JOIN exercises 验证归属）
    let old = sqlx::query_as::<_, Record>(
        "SELECT r.* FROM records r\n        INNER JOIN exercises e ON r.exercise_id = e.id\n        WHERE r.id = ? AND e.user_id = ?",
    )
    .bind(&id)
    .bind(&user.id)
    .fetch_optional(&pool)
    .await
    .map_err(ApiError::Database)?
    .ok_or_else(|| ApiError::NotFound("记录不存在".to_string()))?;

    let weight = req.weight.unwrap_or(old.weight);
    let sets = req.sets.unwrap_or(old.sets);
    let reps = req.reps.unwrap_or(old.reps);
    let rest = req.rest.unwrap_or(old.rest);
    let feeling = req.feeling.unwrap_or(old.feeling.clone());
    let strategy = req.strategy.unwrap_or(old.strategy.clone());
    let key_points = req.key_points.unwrap_or(old.key_points.clone());
    let completed = req.completed.unwrap_or(old.completed);

    if weight < 0.0 || sets < 0 || reps < 0 || rest < 0
    {
        return Err(ApiError::Validation(
            "重量/组数/次数/休息不能为负数".to_string(),
        ));
    }

    let saved = sqlx::query_as::<_, Record>(
        "UPDATE records SET weight = ?, sets = ?, reps = ?, rest = ?,
        feeling = ?, strategy = ?, key_points = ?, completed = ?
        WHERE id = ?
        RETURNING *",
    )
    .bind(&weight)
    .bind(&sets)
    .bind(&reps)
    .bind(&rest)
    .bind(&feeling)
    .bind(&strategy)
    .bind(&key_points)
    .bind(&completed)
    .bind(&id)
    .fetch_one(&pool)
    .await
    .map_err(ApiError::Database)?;

    Ok(Json(record_out(&pool, &saved, user.id).await?))
}

// ============================================================
// 删除记录（DELETE /api/v1/records/{id}）
// ============================================================
/// 删除某条记录
///
/// 【实现步骤】
/// 1. DELETE FROM records WHERE id = ? AND user_id = ?
/// 2. rows_affected() == 0 → NotFound
/// 3. 返回 {"ok": true}
pub async fn delete_record(
    State(state): State<AppState>,
    ApiAuthUser(user): ApiAuthUser,
    Path(id): Path<i64>,
) -> Result<Json<serde_json::Value>, ApiError>
{
    let pool = state.pool.read().await.clone();

    // 数据隔离：先 JOIN exercises 验证归属，再删（records 无 user_id 列）
    let owned = sqlx::query_scalar::<_, i64>(
        "SELECT r.id FROM records r\n        INNER JOIN exercises e ON r.exercise_id = e.id\n        WHERE r.id = ? AND e.user_id = ?",
    )
    .bind(&id)
    .bind(&user.id)
    .fetch_optional(&pool)
    .await
    .map_err(ApiError::Database)?;

    if owned.is_none()
    {
        return Err(ApiError::NotFound("记录不存在".to_string()));
    }

    let ret = sqlx::query("DELETE FROM records WHERE id = ?")
        .bind(&id)
        .execute(&pool)
        .await
        .map_err(ApiError::Database)?;

    if ret.rows_affected() == 0
    {
        return Err(ApiError::NotFound("记录不存在".to_string()));
    }

    Ok(Json(serde_json::json!({ "ok": true })))
}

// ============================================================
// 【教学：日期格式校验 —— YYYY-MM-DD】
// ============================================================
// 同 plans.rs 的 validate_date：拆三段，每段 parse 数字。
// 失败 → Validation（400）。
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
