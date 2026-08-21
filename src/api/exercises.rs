// ============================================================
// api/exercises.rs —— 动作库 API（M8 第 4 步）
// ============================================================
// 【教学说明】
// 动作库 CRUD，JSON 输入输出。端点：
//
//   GET    /api/v1/exercises?body_part=胸    动作列表（可按部位筛选）
//   POST   /api/v1/exercises                 创建动作
//   GET    /api/v1/exercises/{id}            动作详情（含 1RM 统计）
//   PATCH  /api/v1/exercises/{id}            更新动作
//   DELETE /api/v1/exercises/{id}            删除动作
//
// 【教学：与页面层的差异】
//   页面 create/update 用 Form（urlencoded，字段全 String），
//   API 用 Json（serde_json 自动解析数字类型，无需手动 parse）。
//   页面表单"留空提交 "" 导致 f64 400"的坑在 API 层不存在：
//   客户端传 JSON 数字，serde 直接给 f64/i64，类型安全。
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
    models::Exercise,
};

// ⚠️ 注意：exercise_out 挖空后 Record / epley_1rm 的 import 已删，
//    你实现时需要加回：
//     use crate::calc::epley_1rm;
//     models::{Exercise, Record}

// ============================================================
// 【教学：ExerciseOut —— 动作 DTO】
// ============================================================
// 比 models::Exercise 多一个 last_record 摘要（最近一次训练）：
//   date：最近训练日期
//   best_1rm：该动作历史最高 1RM（实时计算，不落库）
// 为什么"最近记录"不在 models 里？它是派生数据，查询时算。
#[derive(Serialize)]
pub struct ExerciseOut
{
    pub id: i64,
    pub name: String,
    pub body_part: String,
    pub default_mode: String,
    pub bar_weight: f64,
    pub default_unit: String,
    pub default_sets: i64,
    pub default_reps: i64,
    pub key_points: String,
    /// 最近训练日期（无记录 → None）
    pub last_record_date: Option<String>,
    /// 历史最高 1RM（无记录 → None）
    pub best_1rm: Option<f64>,
}

// ============================================================
// 【教学：ExerciseCreateReq —— 创建动作请求体】
// ============================================================
// 与页面 ExerciseForm 对应，但数字字段直接用 f64/i64（JSON 类型安全）。
// bar_weight 可空？页面默认 20.0。API 客户端不传 → 默认 20.0（杠铃）。
#[derive(Deserialize)]
pub struct ExerciseCreateReq
{
    pub name: String,
    pub body_part: String,
    #[serde(default = "default_mode")]
    pub default_mode: String,
    #[serde(default = "default_bar_weight")]
    pub bar_weight: f64,
    #[serde(default = "default_unit")]
    pub default_unit: String,
    #[serde(default = "default_sets")]
    pub default_sets: i64,
    #[serde(default = "default_reps")]
    pub default_reps: i64,
    #[serde(default)]
    pub key_points: String,
}

fn default_mode() -> String
{
    "bar".to_string()
}
fn default_bar_weight() -> f64
{
    20.0
}
fn default_unit() -> String
{
    "kg".to_string()
}
fn default_sets() -> i64
{
    3
}
fn default_reps() -> i64
{
    8
}

// ============================================================
// 【教学：Exercise → ExerciseOut 转换】
// ============================================================
// 派生数据（last_record_date / best_1rm）要查 records 表，
// 所以是 async 函数（不能 From）。
// ⚠️ 挖空练习期间加 allow 消除 unused 警告，实现完成后可删
#[allow(unused)]
async fn exercise_out(pool: &SqlitePool, ex: &Exercise) -> Result<ExerciseOut, ApiError>
{
    // 【实现步骤】
    // 1. 查该动作全部记录（升序）：
    //      SELECT * FROM records WHERE exercise_id = ? ORDER BY record_date ASC, id ASC
    // 2. last_record_date：records.last() 的 record_date（升序 → last 即最新）
    // 3. best_1rm：records.iter().map(epley_1rm(weight, reps))
    //      .fold(0.0f64, f64::max) → .pipe(|v| if v > 0.0 { Some(v) } else { None })
    //    （epley_1rm 无效输入返回 0.0 → 过滤成 None）
    // 4. 组装 ExerciseOut（字段照抄 ex + 上面两个派生值）
    todo!("M8 练习：exercise_out 实现") // 【待实现】
}

// 【教学：pipe —— 把值喂给闭包（标准库没有，用局部函数替代）】
// fold 之后的值要做"0.0 → None"的清理。Rust 标准库没有 pipe，
// 这里用一个局部泛型函数实现同样的"值 → 转换"流：
// （⚠️ exercise_out 挖空期间暂时无使用方，加 allow 消除 dead_code 警告；
//    你实现 exercise_out 后可以删掉这个属性）
#[allow(dead_code)]
trait Pipe: Sized
{
    fn pipe<R>(self, f: impl FnOnce(Self) -> R) -> R;
}
impl<T: Sized> Pipe for T
{
    fn pipe<R>(self, f: impl FnOnce(Self) -> R) -> R
    {
        f(self)
    }
}

// ============================================================
// 动作列表（GET /api/v1/exercises?body_part=胸）
// ============================================================
/// 动作列表（可按部位筛选）
///
/// 【教学：Query 提取器 —— 可选查询参数】
/// ?body_part=胸 → Some("胸")；不带参数 → None
/// 页面层踩过的坑（空串筛选 = 全部）：API 同样处理——
/// 空串 → 视为不筛选。
///
/// 【实现步骤】
/// 1. 签名：State + ApiAuthUser + Query<ListQuery>
/// 2. part_filter：query.body_part.as_deref().filter(|p| !p.is_empty())
/// 3. match part_filter：Some → 带条件查；None → 查全部
/// 4. 迭代器转 ExerciseOut（每个查派生数据）
#[derive(Deserialize)]
pub struct ListQuery
{
    pub body_part: Option<String>,
}

pub async fn list(
    State(state): State<AppState>,
    ApiAuthUser(user): ApiAuthUser,
    Query(query): Query<ListQuery>,
) -> Result<Json<Vec<ExerciseOut>>, ApiError>
{
    let pool = state.pool.read().await.clone();

    // 空串筛选 = 不筛选（页面层同款）
    let part_filter = query.body_part.as_deref().filter(|p| !p.is_empty());

    let exercises = match part_filter
    {
        None => sqlx::query_as::<_, Exercise>(
            "SELECT * FROM exercises WHERE user_id = ? ORDER BY body_part, sort_order, id",
        )
        .bind(&user.id)
        .fetch_all(&pool),
        Some(pt) => sqlx::query_as::<_, Exercise>(
            "SELECT * FROM exercises WHERE user_id = ? AND body_part = ? ORDER BY sort_order, id",
        )
        .bind(&user.id)
        .bind(pt)
        .fetch_all(&pool),
    }
    .await
    .map_err(ApiError::Database)?;

    let mut out = Vec::with_capacity(exercises.len());
    for ex in &exercises
    {
        out.push(exercise_out(&pool, ex).await?);
    }

    Ok(Json(out))
}

// ============================================================
// 创建动作（POST /api/v1/exercises）
// ============================================================
/// 创建动作 → 返回新动作 JSON（含 id）
///
/// 【教学：与页面 create 的差异】
/// 页面 create：字段全 String，parse 数字（空串会 400）
/// API create ：serde 直接给数字类型（JSON 类型安全，无空串问题）
///
/// 【实现步骤】
/// 1. 签名：State + ApiAuthUser + Json<ExerciseCreateReq>
/// 2. 校验：name 非空、body_part 非空
/// 3. INSERT INTO exercises (...) VALUES (?, ?, ...) RETURNING id
/// 4. 查完整行 → exercise_out → Json
pub async fn create(
    State(state): State<AppState>,
    ApiAuthUser(user): ApiAuthUser,
    Json(req): Json<ExerciseCreateReq>,
) -> Result<Json<ExerciseOut>, ApiError>
{
    let pool = state.pool.read().await.clone();

    if req.name.trim().is_empty() || req.body_part.trim().is_empty()
    {
        return Err(ApiError::Validation("动作名和部位不能为空".to_string()));
    }

    // 查重（数据隔离 + 防重名，和页面 create 同款）
    if sqlx::query_scalar::<_, i64>("SELECT id FROM exercises WHERE user_id = ? AND name = ?")
        .bind(&user.id)
        .bind(&req.name)
        .fetch_optional(&pool)
        .await
        .map_err(ApiError::Database)?
        .is_some()
    {
        return Err(ApiError::Validation("动作名已存在".to_string()));
    }

    let new_id = sqlx::query_scalar::<_, i64>(
        "INSERT INTO exercises
        (user_id, name, body_part, default_mode, bar_weight, default_unit,
         default_sets, default_reps, key_points, sort_order)
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, 0)
        RETURNING id",
    )
    .bind(&user.id)
    .bind(&req.name)
    .bind(&req.body_part)
    .bind(&req.default_mode)
    .bind(&req.bar_weight)
    .bind(&req.default_unit)
    .bind(&req.default_sets)
    .bind(&req.default_reps)
    .bind(&req.key_points)
    .fetch_one(&pool)
    .await
    .map_err(ApiError::Database)?;

    let ex = sqlx::query_as::<_, Exercise>("SELECT * FROM exercises WHERE id = ? AND user_id = ?")
        .bind(&new_id)
        .bind(&user.id)
        .fetch_one(&pool)
        .await
        .map_err(ApiError::Database)?;

    Ok(Json(exercise_out(&pool, &ex).await?))
}

// ============================================================
// 【教学：ExerciseUpdateReq —— PATCH 请求体】
// ============================================================
// 部分更新：缺字段 → 用旧值。数字字段 Option<f64>（null 视为不改）。
#[derive(Deserialize)]
pub struct ExerciseUpdateReq
{
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub body_part: Option<String>,
    #[serde(default)]
    pub default_mode: Option<String>,
    #[serde(default)]
    pub bar_weight: Option<f64>,
    #[serde(default)]
    pub default_unit: Option<String>,
    #[serde(default)]
    pub default_sets: Option<i64>,
    #[serde(default)]
    pub default_reps: Option<i64>,
    #[serde(default)]
    pub key_points: Option<String>,
}

// ============================================================
// 动作详情（GET /api/v1/exercises/{id}）
// ============================================================
/// 动作详情（含最近训练 + 最高 1RM）
pub async fn detail(
    State(state): State<AppState>,
    ApiAuthUser(user): ApiAuthUser,
    Path(id): Path<i64>,
) -> Result<Json<ExerciseOut>, ApiError>
{
    let pool = state.pool.read().await.clone();

    let ex = sqlx::query_as::<_, Exercise>("SELECT * FROM exercises WHERE id = ? AND user_id = ?")
        .bind(&id)
        .bind(&user.id)
        .fetch_optional(&pool)
        .await
        .map_err(ApiError::Database)?
        .ok_or_else(|| ApiError::NotFound("动作不存在".to_string()))?;

    Ok(Json(exercise_out(&pool, &ex).await?))
}

// ============================================================
// 更新动作（PATCH /api/v1/exercises/{id}）
// ============================================================
/// 更新动作（部分更新，先查旧值合并）
pub async fn update(
    State(state): State<AppState>,
    ApiAuthUser(user): ApiAuthUser,
    Path(id): Path<i64>,
    Json(req): Json<ExerciseUpdateReq>,
) -> Result<Json<ExerciseOut>, ApiError>
{
    let pool = state.pool.read().await.clone();

    let old = sqlx::query_as::<_, Exercise>("SELECT * FROM exercises WHERE id = ? AND user_id = ?")
        .bind(&id)
        .bind(&user.id)
        .fetch_optional(&pool)
        .await
        .map_err(ApiError::Database)?
        .ok_or_else(|| ApiError::NotFound("动作不存在".to_string()))?;

    let name = req.name.unwrap_or(old.name);
    let body_part = req.body_part.unwrap_or(old.body_part);
    let default_mode = req.default_mode.unwrap_or(old.default_mode);
    let bar_weight = req.bar_weight.unwrap_or(old.bar_weight);
    let default_unit = req.default_unit.unwrap_or(old.default_unit);
    let default_sets = req.default_sets.unwrap_or(old.default_sets);
    let default_reps = req.default_reps.unwrap_or(old.default_reps);
    let key_points = req.key_points.unwrap_or(old.key_points);

    if name.trim().is_empty() || body_part.trim().is_empty()
    {
        return Err(ApiError::Validation("动作名和部位不能为空".to_string()));
    }

    let ret = sqlx::query(
        "UPDATE exercises SET name = ?, body_part = ?, default_mode = ?, bar_weight = ?,
         default_unit = ?, default_sets = ?, default_reps = ?, key_points = ?
         WHERE id = ? AND user_id = ?",
    )
    .bind(&name)
    .bind(&body_part)
    .bind(&default_mode)
    .bind(&bar_weight)
    .bind(&default_unit)
    .bind(&default_sets)
    .bind(&default_reps)
    .bind(&key_points)
    .bind(&id)
    .bind(&user.id)
    .execute(&pool)
    .await
    .map_err(ApiError::Database)?;

    if ret.rows_affected() == 0
    {
        return Err(ApiError::NotFound("动作不存在".to_string()));
    }

    let ex = sqlx::query_as::<_, Exercise>("SELECT * FROM exercises WHERE id = ? AND user_id = ?")
        .bind(&id)
        .bind(&user.id)
        .fetch_one(&pool)
        .await
        .map_err(ApiError::Database)?;

    Ok(Json(exercise_out(&pool, &ex).await?))
}

// ============================================================
// 删除动作（DELETE /api/v1/exercises/{id}）
// ============================================================
/// 删除动作
///
/// 【教学：删除动作的引用问题（页面层遗留的演进点）】
/// 页面 delete 直接 DELETE，没有检查引用（template_items/plan_items/records
/// 都引用 exercise_id）。API 层也一样——但注意：
///   若动作已被模板/计划/记录引用，SQLite 外键约束会报错（500）。
/// 这是设计取舍：M8 保持与页面一致（直接删），
/// 引用检查（有引用则拒绝）留作未来增强。
///
/// 【实现步骤】
/// 1. 签名：State + ApiAuthUser + Path(id)
/// 2. DELETE FROM exercises WHERE id = ? AND user_id = ?
/// 3. rows_affected() == 0 → NotFound
/// 4. 返回 {"ok": true}（或删除的对象——M8 简化返回 ok）
pub async fn delete(
    State(state): State<AppState>,
    ApiAuthUser(user): ApiAuthUser,
    Path(id): Path<i64>,
) -> Result<Json<serde_json::Value>, ApiError>
{
    let pool = state.pool.read().await.clone();

    let ret = sqlx::query("DELETE FROM exercises WHERE id = ? AND user_id = ?")
        .bind(&id)
        .bind(&user.id)
        .execute(&pool)
        .await
        .map_err(ApiError::Database)?;

    if ret.rows_affected() == 0
    {
        return Err(ApiError::NotFound("动作不存在".to_string()));
    }

    Ok(Json(serde_json::json!({ "ok": true })))
}
