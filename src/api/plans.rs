// ============================================================
// api/plans.rs —— 模板 + 计划 API（M8 第 5 步）
// ============================================================
// 【教学说明】
// 模板（Template）和计划（Plan）都是"父表 + 子表（items）"结构：
//   templates ─┬─ template_items（模板项）
//   plans ─────┴─ plan_items（计划项）
//
// 端点：
//
//   GET   /api/v1/phases/{phase_id}/templates          模板列表
//   POST  /api/v1/phases/{phase_id}/templates          创建模板（含动作项）
//   PATCH /api/v1/templates/{id}                       更新模板
//   DELETE /api/v1/templates/{id}                      删除模板
//   GET   /api/v1/phases/{phase_id}/plans?date=YYYY-MM-DD  计划列表/按日期
//   POST  /api/v1/phases/{phase_id}/plans              创建计划
//   GET   /api/v1/plans/{id}                           计划详情（含动作项）
//   PATCH /api/v1/plans/{id}                           更新计划
//   DELETE /api/v1/plans/{id}                          删除计划
//
// 【教学：JSON 数组传 items —— 表单多选坑的天然解药】
// 页面层用 checkbox 多选（todo.md §2.1 的 serde_urlencoded map 语义坑），
// API 层直接传 JSON 数组：
//   {"name": "推日", "items": [{"exercise_id": 6}, {"exercise_id": 7}]}
// items 是 Vec，顺序 = 数组顺序，不存在"后值覆盖前值"。
// 顺序落库：enumerate() 生成 sort_order。
//
// 📌 阶段要求：M8 你来实现本文件所有函数。
//   完整实现已备份在 docs/learning_path/M8_ref/，实现完成后对照检查。
// ============================================================
use std::collections::HashMap;

use axum::{
    Json,
    extract::{Path, Query, State},
};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

use crate::{
    AppState,
    api::{ApiError, auth::ApiAuthUser},
    models::{Exercise, Phase, Plan, PlanItem, Template, TemplateItem},
};

// ============================================================
// 【教学：DTO 结构体】
// ============================================================
// TemplateOut：模板 + 动作项（每项含动作名）
// PlanOut：计划 + 动作项（每项含动作名/部位）
// 动作名要 JOIN 查询（查两次 + HashMap 索引，和页面层同款）。
#[derive(Serialize)]
pub struct TemplateItemOut
{
    pub id: i64,
    pub exercise_id: i64,
    pub exercise_name: String,
    pub plan_sets: Option<i64>,
    pub plan_reps: Option<i64>,
}

#[derive(Serialize)]
pub struct TemplateOut
{
    pub id: i64,
    pub phase_id: i64,
    pub name: String,
    pub items: Vec<TemplateItemOut>,
}

#[derive(Serialize)]
pub struct PlanItemOut
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
    pub plan_note: Option<String>,
}

#[derive(Serialize)]
pub struct PlanOut
{
    pub id: i64,
    pub phase_id: i64,
    pub date: String,
    pub note: String,
    pub items: Vec<PlanItemOut>,
}

// ============================================================
// 【教学：请求体结构体】
// ============================================================
// TemplateCreateReq：name + items（exercise_id 数组）
// PlanCreateReq：date + note + items（完整计划项字段）
// 更新用同款（PATCH 简化：M8 要求全量提交 name/items，和页面编辑一致）
#[derive(Deserialize)]
pub struct TemplateReq
{
    pub name: String,
    #[serde(default)]
    pub items: Vec<TemplateItemReq>,
}

#[derive(Deserialize)]
pub struct TemplateItemReq
{
    pub exercise_id: i64,
}

#[derive(Deserialize)]
pub struct PlanReq
{
    pub date: String,
    #[serde(default)]
    pub note: String,
    #[serde(default)]
    pub items: Vec<PlanItemReq>,
}

#[derive(Deserialize)]
pub struct PlanItemReq
{
    pub exercise_id: i64,
    #[serde(default)]
    pub plan_sets: Option<i64>,
    #[serde(default)]
    pub plan_reps: Option<i64>,
    #[serde(default)]
    pub plan_weight: Option<f64>,
    #[serde(default)]
    pub plan_rest: Option<i64>,
    #[serde(default)]
    pub plan_key_points: Option<String>,
    #[serde(default)]
    pub plan_note: Option<String>,
}

// ============================================================
// 【教学：验证阶段归属（数据隔离第一步）】
// ============================================================
// 所有"阶段下"的操作（模板/计划列表、创建）都要先验证：
//   1. 阶段存在且属于当前用户（WHERE id = ? AND user_id = ?）
//   2. 未归档（archived = 0，归档阶段只读）
// 返回 Phase（调用方可能还要用 phase_id/name）。
async fn verify_phase(pool: &SqlitePool, user_id: i64, phase_id: i64) -> Result<Phase, ApiError>
{
    let phase = sqlx::query_as::<_, Phase>("SELECT * FROM phases WHERE id = ? AND user_id = ?")
        .bind(&phase_id)
        .bind(&user_id)
        .fetch_optional(pool)
        .await
        .map_err(ApiError::Database)?
        .ok_or_else(|| ApiError::NotFound("阶段不存在".to_string()))?;

    if phase.archived
    {
        return Err(ApiError::Forbidden("归档阶段不可编辑".to_string()));
    }

    Ok(phase)
}

// ============================================================
// 【教学：验证模板/计划归属（JOIN phases 拿 user_id）】
// ============================================================
// 按模板 id / 计划 id 操作时，模板表本身没有 user_id，
// 必须 JOIN phases 验证归属（页面层同款 SQL）：
//   SELECT t.* FROM templates t INNER JOIN phases p ON t.phase_id = p.id
//   WHERE t.id = ? AND p.user_id = ?
// 返回模板（phase_id 供后续使用）。
async fn verify_template(
    pool: &SqlitePool,
    user_id: i64,
    template_id: i64,
) -> Result<Template, ApiError>
{
    sqlx::query_as::<_, Template>(
        "SELECT t.* FROM templates t INNER JOIN phases p ON t.phase_id = p.id
    WHERE t.id = ? AND p.user_id = ?",
    )
    .bind(&template_id)
    .bind(&user_id)
    .fetch_optional(pool)
    .await
    .map_err(ApiError::Database)?
    .ok_or_else(|| ApiError::NotFound("模板不存在".to_string()))
}

async fn verify_plan(pool: &SqlitePool, user_id: i64, plan_id: i64) -> Result<Plan, ApiError>
{
    sqlx::query_as::<_, Plan>(
        "SELECT p.* FROM plans p INNER JOIN phases ph ON p.phase_id = ph.id
    WHERE p.id = ? AND ph.user_id = ?",
    )
    .bind(&plan_id)
    .bind(&user_id)
    .fetch_optional(pool)
    .await
    .map_err(ApiError::Database)?
    .ok_or_else(|| ApiError::NotFound("计划不存在".to_string()))
}

// ============================================================
// 【教学：组装 TemplateOut —— 模板 + 动作项 + 动作名索引】
// ============================================================
// 查两次 + HashMap 索引（页面层同款模式）：
//   1. SELECT * FROM template_items WHERE template_id = ? ORDER BY sort_order
//   2. SELECT * FROM exercises WHERE user_id = ? → id → name 索引
// 为什么不用 JOIN？query_as 按列名匹配结构体，JOIN 多出的列不匹配。
async fn template_out(
    pool: &SqlitePool,
    t: &Template,
    user_id: i64,
) -> Result<TemplateOut, ApiError>
{
    let items = sqlx::query_as::<_, TemplateItem>(
        "SELECT * FROM template_items WHERE template_id = ? ORDER BY sort_order",
    )
    .bind(&t.id)
    .fetch_all(pool)
    .await
    .map_err(ApiError::Database)?;

    let ex_names: HashMap<i64, String> =
        sqlx::query_as::<_, Exercise>("SELECT * FROM exercises WHERE user_id = ?")
            .bind(&user_id)
            .fetch_all(pool)
            .await
            .map_err(ApiError::Database)?
            .into_iter()
            .map(|e| (e.id, e.name))
            .collect();

    let items_out = items
        .iter()
        .map(|i| TemplateItemOut {
            id: i.id,
            exercise_id: i.exercise_id,
            exercise_name: ex_names
                .get(&i.exercise_id)
                .cloned()
                .unwrap_or_else(|| "未知动作".to_string()),
            plan_sets: i.plan_sets,
            plan_reps: i.plan_reps,
        })
        .collect();

    Ok(TemplateOut {
        id: t.id,
        phase_id: t.phase_id,
        name: t.name.clone(),
        items: items_out,
    })
}

async fn plan_out(pool: &SqlitePool, p: &Plan, user_id: i64) -> Result<PlanOut, ApiError>
{
    let items = sqlx::query_as::<_, PlanItem>(
        "SELECT * FROM plan_items WHERE plan_id = ? ORDER BY sort_order",
    )
    .bind(&p.id)
    .fetch_all(pool)
    .await
    .map_err(ApiError::Database)?;

    let ex_names: HashMap<i64, (String, String)> =
        sqlx::query_as::<_, Exercise>("SELECT * FROM exercises WHERE user_id = ?")
            .bind(&user_id)
            .fetch_all(pool)
            .await
            .map_err(ApiError::Database)?
            .into_iter()
            .map(|e| (e.id, (e.name, e.body_part)))
            .collect();

    let items_out = items
        .iter()
        .map(|i| {
            let (name, part) = ex_names
                .get(&i.exercise_id)
                .cloned()
                .unwrap_or_else(|| ("未知动作".to_string(), "未分组".to_string()));
            PlanItemOut {
                id: i.id,
                exercise_id: i.exercise_id,
                exercise_name: name,
                body_part: part,
                plan_sets: i.plan_sets,
                plan_reps: i.plan_reps,
                plan_weight: i.plan_weight,
                plan_rest: i.plan_rest,
                plan_key_points: i.plan_key_points.clone(),
                plan_note: i.plan_note.clone(),
            }
        })
        .collect();

    Ok(PlanOut {
        id: p.id,
        phase_id: p.phase_id,
        date: p.date.clone(),
        note: p.note.clone(),
        items: items_out,
    })
}

// ============================================================
// 模板列表（GET /api/v1/phases/{phase_id}/templates）
// ============================================================
/// 阶段下的模板列表（每个含动作项）
///
/// 【实现步骤】
/// 1. 验证阶段归属（verify_phase，但列表不要求未归档——归档阶段也能看）
/// 2. SELECT * FROM templates WHERE phase_id = ? ORDER BY sort_order
/// 3. 逐个转 TemplateOut
pub async fn template_list(
    State(state): State<AppState>,
    ApiAuthUser(user): ApiAuthUser,
    Path(phase_id): Path<i64>,
) -> Result<Json<Vec<TemplateOut>>, ApiError>
{
    let pool = state.pool.read().await.clone();

    // 列表只验证存在 + 归属（归档阶段也能查看列表）
    sqlx::query_as::<_, Phase>("SELECT * FROM phases WHERE id = ? AND user_id = ?")
        .bind(&phase_id)
        .bind(&user.id)
        .fetch_optional(&pool)
        .await
        .map_err(ApiError::Database)?
        .ok_or_else(|| ApiError::NotFound("阶段不存在".to_string()))?;

    let templates = sqlx::query_as::<_, Template>(
        "SELECT * FROM templates WHERE phase_id = ? ORDER BY sort_order, id",
    )
    .bind(&phase_id)
    .fetch_all(&pool)
    .await
    .map_err(ApiError::Database)?;

    let mut out = Vec::with_capacity(templates.len());
    for t in &templates
    {
        out.push(template_out(&pool, t, user.id).await?);
    }

    Ok(Json(out))
}

// ============================================================
// 创建模板（POST /api/v1/phases/{phase_id}/templates）
// ============================================================
/// 创建模板（含动作项）→ 返回新模板 JSON
///
/// 【教学：事务 —— 多张表要么全成要么全败】
/// INSERT templates + 循环 INSERT template_items，必须在一个事务里：
/// 遗漏 commit 会全部回滚（AGENTS.md 事务纪律）！
///
/// 【实现步骤】
/// 1. verify_phase（归属 + 未归档）
/// 2. 校验：name 非空、items 非空（至少一个动作）
/// 3. 计算 next_sort：SELECT COALESCE(MAX(sort_order), -1) + 1 FROM templates WHERE phase_id = ?
/// 4. begin → INSERT templates RETURNING id → 循环 INSERT items（enumerate 生成 sort_order）
/// 5. commit → template_out → Json
pub async fn template_create(
    State(state): State<AppState>,
    ApiAuthUser(user): ApiAuthUser,
    Path(phase_id): Path<i64>,
    Json(req): Json<TemplateReq>,
) -> Result<Json<TemplateOut>, ApiError>
{
    let pool = state.pool.read().await.clone();

    // 归属 + 未归档验证
    verify_phase(&pool, user.id, phase_id).await?;

    // 校验
    if req.name.trim().is_empty()
    {
        return Err(ApiError::Validation("模板名称不能为空".to_string()));
    }
    if req.items.is_empty()
    {
        return Err(ApiError::Validation("至少选择一个动作".to_string()));
    }

    // 下一个排序号
    let next_sort = sqlx::query_scalar::<_, i64>(
        "SELECT COALESCE(MAX(sort_order), -1) + 1 FROM templates WHERE phase_id = ?",
    )
    .bind(&phase_id)
    .fetch_one(&pool)
    .await
    .map_err(ApiError::Database)?;

    // 事务：父表 + 子表
    let mut tx = pool.begin().await.map_err(ApiError::Database)?;

    let template_id = sqlx::query_scalar::<_, i64>(
        "INSERT INTO templates (phase_id, name, sort_order) VALUES (?, ?, ?)
        RETURNING id",
    )
    .bind(&phase_id)
    .bind(&req.name)
    .bind(next_sort)
    .fetch_one(&mut *tx)
    .await
    .map_err(ApiError::Database)?;

    for (idx, item) in req.items.iter().enumerate()
    {
        sqlx::query(
            "INSERT INTO template_items (template_id, exercise_id, sort_order) VALUES (?, ?, ?)",
        )
        .bind(&template_id)
        .bind(item.exercise_id)
        .bind(idx as i64)
        .execute(&mut *tx)
        .await
        .map_err(ApiError::Database)?;
    }

    tx.commit().await.map_err(ApiError::Database)?;

    // 查完整模板返回
    let template =
        sqlx::query_as::<_, Template>("SELECT * FROM templates WHERE id = ? AND phase_id = ?")
            .bind(&template_id)
            .bind(&phase_id)
            .fetch_one(&pool)
            .await
            .map_err(ApiError::Database)?;

    Ok(Json(template_out(&pool, &template, user.id).await?))
}

// ============================================================
// 更新模板（PATCH /api/v1/templates/{id}）
// ============================================================
/// 更新模板（改名 + 换动作集合）
///
/// 【教学：更新子表集合的标准套路 —— "先删后插"】
/// 1. 更新父表（改名）
/// 2. 删掉所有旧子表行
/// 3. 重新插入（顺序 = 请求数组顺序）
/// 三步一个事务（页面层 template_update 同款）。
///
/// 【实现步骤】
/// 1. verify_template（归属）→ verify_phase（未归档）
/// 2. 校验 name/items
/// 3. begin → UPDATE templates → DELETE template_items → 循环 INSERT
/// 4. commit → template_out → Json
pub async fn template_update(
    State(state): State<AppState>,
    ApiAuthUser(user): ApiAuthUser,
    Path(id): Path<i64>,
    Json(req): Json<TemplateReq>,
) -> Result<Json<TemplateOut>, ApiError>
{
    let pool = state.pool.read().await.clone();

    // 归属验证（拿 phase_id）
    let tpl = verify_template(&pool, user.id, id).await?;
    // 未归档验证
    verify_phase(&pool, user.id, tpl.phase_id).await?;

    if req.name.trim().is_empty()
    {
        return Err(ApiError::Validation("模板名称不能为空".to_string()));
    }
    if req.items.is_empty()
    {
        return Err(ApiError::Validation("至少选择一个动作".to_string()));
    }

    let mut tx = pool.begin().await.map_err(ApiError::Database)?;

    sqlx::query("UPDATE templates SET name = ? WHERE id = ?")
        .bind(&req.name)
        .bind(&id)
        .execute(&mut *tx)
        .await
        .map_err(ApiError::Database)?;

    // 先删后插
    sqlx::query("DELETE FROM template_items WHERE template_id = ?")
        .bind(&id)
        .execute(&mut *tx)
        .await
        .map_err(ApiError::Database)?;

    for (idx, item) in req.items.iter().enumerate()
    {
        sqlx::query(
            "INSERT INTO template_items (template_id, exercise_id, sort_order) VALUES (?, ?, ?)",
        )
        .bind(&id)
        .bind(item.exercise_id)
        .bind(idx as i64)
        .execute(&mut *tx)
        .await
        .map_err(ApiError::Database)?;
    }

    tx.commit().await.map_err(ApiError::Database)?;

    let template = sqlx::query_as::<_, Template>("SELECT * FROM templates WHERE id = ?")
        .bind(&id)
        .fetch_one(&pool)
        .await
        .map_err(ApiError::Database)?;

    Ok(Json(template_out(&pool, &template, user.id).await?))
}

// ============================================================
// 删除模板（DELETE /api/v1/templates/{id}）
// ============================================================
/// 删除模板（连同它的所有模板项）
///
/// 【教学：先子后父】
/// DELETE template_items（孩子）→ DELETE templates（父亲）
/// 顺序不能反，否则父表删了子表留孤儿数据。
pub async fn template_delete(
    State(state): State<AppState>,
    ApiAuthUser(user): ApiAuthUser,
    Path(id): Path<i64>,
) -> Result<Json<serde_json::Value>, ApiError>
{
    let pool = state.pool.read().await.clone();

    // 归属验证
    verify_template(&pool, user.id, id).await?;

    let mut tx = pool.begin().await.map_err(ApiError::Database)?;

    sqlx::query("DELETE FROM template_items WHERE template_id = ?")
        .bind(&id)
        .execute(&mut *tx)
        .await
        .map_err(ApiError::Database)?;

    sqlx::query("DELETE FROM templates WHERE id = ?")
        .bind(&id)
        .execute(&mut *tx)
        .await
        .map_err(ApiError::Database)?;

    tx.commit().await.map_err(ApiError::Database)?;

    Ok(Json(serde_json::json!({ "ok": true })))
}

// ============================================================
// 计划列表（GET /api/v1/phases/{phase_id}/plans?date=YYYY-MM-DD）
// ============================================================
/// 阶段下的计划列表（可按日期筛选）
///
/// 【教学：?date= 可选筛选】
/// 带 date → 只查那天；不带 → 查全部（倒序，最新的在前）。
#[derive(Deserialize)]
pub struct PlanListQuery
{
    pub date: Option<String>,
}

pub async fn plan_list(
    State(state): State<AppState>,
    ApiAuthUser(user): ApiAuthUser,
    Path(phase_id): Path<i64>,
    Query(query): Query<PlanListQuery>,
) -> Result<Json<Vec<PlanOut>>, ApiError>
{
    let pool = state.pool.read().await.clone();

    // 归属验证（归档阶段也能看列表）
    sqlx::query_as::<_, Phase>("SELECT * FROM phases WHERE id = ? AND user_id = ?")
        .bind(&phase_id)
        .bind(&user.id)
        .fetch_optional(&pool)
        .await
        .map_err(ApiError::Database)?
        .ok_or_else(|| ApiError::NotFound("阶段不存在".to_string()))?;

    let plans = match query.date.as_deref().filter(|d| !d.is_empty())
    {
        None => sqlx::query_as::<_, Plan>(
            "SELECT * FROM plans WHERE phase_id = ? ORDER BY date DESC, id DESC",
        )
        .bind(&phase_id)
        .fetch_all(&pool),
        Some(d) => sqlx::query_as::<_, Plan>(
            "SELECT * FROM plans WHERE phase_id = ? AND date = ? ORDER BY date DESC, id DESC",
        )
        .bind(&phase_id)
        .bind(d)
        .fetch_all(&pool),
    }
    .await
    .map_err(ApiError::Database)?;

    let mut out = Vec::with_capacity(plans.len());
    for p in &plans
    {
        out.push(plan_out(&pool, p, user.id).await?);
    }

    Ok(Json(out))
}

// ============================================================
// 创建计划（POST /api/v1/phases/{phase_id}/plans）
// ============================================================
/// 创建计划（含动作项）→ 返回新计划 JSON
///
/// 【教学：日期校验 —— 必须是 YYYY-MM-DD】
/// date 是必填（计划挂在某天）。校验格式（和 stats::history_day 同款）：
///   拆成 [yyyy, mm, dd] 三段，每段 parse 数字。
///
/// 【实现步骤】
/// 1. verify_phase（归属 + 未归档）
/// 2. 校验：date 格式、items 非空
/// 3. begin → INSERT plans RETURNING id → 循环 INSERT plan_items（enumerate）
/// 4. commit → plan_out → Json
pub async fn plan_create(
    State(state): State<AppState>,
    ApiAuthUser(user): ApiAuthUser,
    Path(phase_id): Path<i64>,
    Json(req): Json<PlanReq>,
) -> Result<Json<PlanOut>, ApiError>
{
    let pool = state.pool.read().await.clone();

    verify_phase(&pool, user.id, phase_id).await?;

    // 日期格式校验
    validate_date(&req.date)?;

    if req.items.is_empty()
    {
        return Err(ApiError::Validation("至少选择一个动作".to_string()));
    }

    let mut tx = pool.begin().await.map_err(ApiError::Database)?;

    let plan_id = sqlx::query_scalar::<_, i64>(
        "INSERT INTO plans (phase_id, date, note) VALUES (?, ?, ?)
        RETURNING id",
    )
    .bind(&phase_id)
    .bind(&req.date)
    .bind(&req.note)
    .fetch_one(&mut *tx)
    .await
    .map_err(ApiError::Database)?;

    for (idx, item) in req.items.iter().enumerate()
    {
        sqlx::query(
            "INSERT INTO plan_items
            (plan_id, exercise_id, sort_order, plan_sets, plan_reps, plan_weight,
             plan_rest, plan_key_points, plan_note)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&plan_id)
        .bind(item.exercise_id)
        .bind(idx as i64)
        .bind(item.plan_sets)
        .bind(item.plan_reps)
        .bind(item.plan_weight)
        .bind(item.plan_rest)
        .bind(&item.plan_key_points)
        .bind(&item.plan_note)
        .execute(&mut *tx)
        .await
        .map_err(ApiError::Database)?;
    }

    tx.commit().await.map_err(ApiError::Database)?;

    let plan = sqlx::query_as::<_, Plan>("SELECT * FROM plans WHERE id = ? AND phase_id = ?")
        .bind(&plan_id)
        .bind(&phase_id)
        .fetch_one(&pool)
        .await
        .map_err(ApiError::Database)?;

    Ok(Json(plan_out(&pool, &plan, user.id).await?))
}

// ============================================================
// 计划详情（GET /api/v1/plans/{id}）
// ============================================================
/// 计划详情（含动作项）
pub async fn plan_detail(
    State(state): State<AppState>,
    ApiAuthUser(user): ApiAuthUser,
    Path(id): Path<i64>,
) -> Result<Json<PlanOut>, ApiError>
{
    let pool = state.pool.read().await.clone();

    let plan = verify_plan(&pool, user.id, id).await?;
    Ok(Json(plan_out(&pool, &plan, user.id).await?))
}

// ============================================================
// 更新计划（PATCH /api/v1/plans/{id}）
// ============================================================
/// 更新计划（改 note + 换动作集合）
///
/// 【教学：⚠️ 外键陷阱 —— 先解除 records 关联再删 plan_items】
/// 已训练过的计划项有 records 引用（records.plan_item_id → plan_items.id）。
/// 直接 DELETE plan_items 会报 FOREIGN KEY constraint failed。
/// 页面层 plan_update 的处理（必须复用）：
///   1. 备份 orphaned：(exercise_id → 该计划下的 record id 列表)
///   2. UPDATE records SET plan_item_id = NULL（解除关联，保留历史）
///   3. DELETE plan_items
///   4. 重插 plan_items（新 id）→ 按备份清单还原 records.plan_item_id
/// 为什么还原？不还原 today 页按 plan_item_id 查记录 → 全部"未训练"。
///
/// 【实现步骤】
/// 1. verify_plan（归属）→ verify_phase（未归档）
/// 2. 校验 date 格式、items 非空
/// 3. begin → 备份 orphaned → 解除 records 关联 → DELETE plan_items
///    → UPDATE plans → 循环 INSERT plan_items（enumerate）→ 还原 records
/// 4. commit → plan_out → Json
pub async fn plan_update(
    State(state): State<AppState>,
    ApiAuthUser(user): ApiAuthUser,
    Path(id): Path<i64>,
    Json(req): Json<PlanReq>,
) -> Result<Json<PlanOut>, ApiError>
{
    let pool = state.pool.read().await.clone();

    let plan = verify_plan(&pool, user.id, id).await?;
    verify_phase(&pool, user.id, plan.phase_id).await?;

    validate_date(&req.date)?;
    if req.items.is_empty()
    {
        return Err(ApiError::Validation("至少选择一个动作".to_string()));
    }

    let mut tx = pool.begin().await.map_err(ApiError::Database)?;

    // ① 备份 orphaned（exercise_id → record id 列表）
    let orphaned: HashMap<i64, Vec<i64>> = sqlx::query_as::<_, (i64, i64)>(
        "SELECT r.exercise_id, r.id FROM records r
        WHERE r.plan_item_id IN (SELECT id FROM plan_items WHERE plan_id = ?)",
    )
    .bind(&id)
    .fetch_all(&mut *tx)
    .await
    .map_err(ApiError::Database)?
    .into_iter()
    .fold(HashMap::new(), |mut acc, (ex_id, rec_id)| {
        acc.entry(ex_id).or_default().push(rec_id);
        acc
    });

    // ② 解除关联（保留训练历史）
    sqlx::query(
        "UPDATE records SET plan_item_id = NULL
        WHERE plan_item_id IN (SELECT id FROM plan_items WHERE plan_id = ?)",
    )
    .bind(&id)
    .execute(&mut *tx)
    .await
    .map_err(ApiError::Database)?;

    // ③ 删旧子表
    sqlx::query("DELETE FROM plan_items WHERE plan_id = ?")
        .bind(&id)
        .execute(&mut *tx)
        .await
        .map_err(ApiError::Database)?;

    // ④ 更新父表
    sqlx::query("UPDATE plans SET date = ?, note = ? WHERE id = ?")
        .bind(&req.date)
        .bind(&req.note)
        .bind(&id)
        .execute(&mut *tx)
        .await
        .map_err(ApiError::Database)?;

    // ⑤ 重插 plan_items + 还原 records 关联
    for (idx, item) in req.items.iter().enumerate()
    {
        let result = sqlx::query(
            "INSERT INTO plan_items
            (plan_id, exercise_id, sort_order, plan_sets, plan_reps, plan_weight,
             plan_rest, plan_key_points, plan_note)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(item.exercise_id)
        .bind(idx as i64)
        .bind(item.plan_sets)
        .bind(item.plan_reps)
        .bind(item.plan_weight)
        .bind(item.plan_rest)
        .bind(&item.plan_key_points)
        .bind(&item.plan_note)
        .execute(&mut *tx)
        .await
        .map_err(ApiError::Database)?;

        let new_item_id = result.last_insert_rowid();

        if let Some(rec_ids) = orphaned.get(&item.exercise_id)
        {
            for rec_id in rec_ids
            {
                sqlx::query("UPDATE records SET plan_item_id = ? WHERE id = ?")
                    .bind(new_item_id)
                    .bind(rec_id)
                    .execute(&mut *tx)
                    .await
                    .map_err(ApiError::Database)?;
            }
        }
    }

    tx.commit().await.map_err(ApiError::Database)?;

    // 查新计划返回
    let updated = sqlx::query_as::<_, Plan>("SELECT * FROM plans WHERE id = ?")
        .bind(&id)
        .fetch_one(&pool)
        .await
        .map_err(ApiError::Database)?;

    Ok(Json(plan_out(&pool, &updated, user.id).await?))
}

// ============================================================
// 删除计划（DELETE /api/v1/plans/{id}）
// ============================================================
/// 删除计划（保留训练记录，解除关联）
///
/// 【教学：页面 plan_delete 同款】
/// 先 UPDATE records SET plan_item_id = NULL（保留历史），
/// 再删 plan_items，最后删 plans（先子后父）。
pub async fn plan_delete(
    State(state): State<AppState>,
    ApiAuthUser(user): ApiAuthUser,
    Path(id): Path<i64>,
) -> Result<Json<serde_json::Value>, ApiError>
{
    let pool = state.pool.read().await.clone();

    verify_plan(&pool, user.id, id).await?;

    let mut tx = pool.begin().await.map_err(ApiError::Database)?;

    // 解除记录关联（保留训练历史）
    sqlx::query(
        "UPDATE records SET plan_item_id = NULL
        WHERE plan_item_id IN (SELECT id FROM plan_items WHERE plan_id = ?)",
    )
    .bind(&id)
    .execute(&mut *tx)
    .await
    .map_err(ApiError::Database)?;

    // 先子后父
    sqlx::query("DELETE FROM plan_items WHERE plan_id = ?")
        .bind(&id)
        .execute(&mut *tx)
        .await
        .map_err(ApiError::Database)?;

    sqlx::query("DELETE FROM plans WHERE id = ?")
        .bind(&id)
        .execute(&mut *tx)
        .await
        .map_err(ApiError::Database)?;

    tx.commit().await.map_err(ApiError::Database)?;

    Ok(Json(serde_json::json!({ "ok": true })))
}

// ============================================================
// 【教学：日期格式校验 —— YYYY-MM-DD】
// ============================================================
// 与 stats::history_day 同款：拆三段，每段 parse 数字。
// 校验失败 → Validation（400）。
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
