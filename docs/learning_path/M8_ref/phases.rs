// ============================================================
// api/phases.rs —— 阶段 API（M8 第 3 步）
// ============================================================
// 【教学说明】
// 页面层已有阶段 CRUD（handlers/phases.rs），API 层做同款操作，
// 但输入输出都是 JSON。端点：
//
//   GET   /api/v1/phases                 阶段列表（含坚持天数）
//   POST  /api/v1/phases                 创建阶段
//   GET   /api/v1/phases/{id}            阶段详情
//   PATCH /api/v1/phases/{id}            更新阶段
//   POST  /api/v1/phases/{id}/archive    归档
//   POST  /api/v1/phases/{id}/unarchive  启用
//
// 【教学：数据隔离纪律（与页面层完全一致）】
// 所有按 id 查询必须带 user_id 条件：
//   SELECT * FROM phases WHERE id = ? AND user_id = ?
// 绝不能只 SELECT * FROM phases WHERE id = ?——那会查到别的用户的阶段！
//
// 【教学：坚持天数计算】
// 页面层用 SQL：SELECT CAST(julianday('now','localtime') - julianday(?) AS INTEGER)
// start_date 为空（未设置）→ days = 0
//
// 📌 阶段要求：M8 你来实现本文件所有函数。
//   完整实现已备份在 docs/learning_path/M8_ref/，实现完成后对照检查。
// ============================================================
use axum::{
    Json,
    extract::{Path, State},
};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

use crate::{
    AppState,
    api::{ApiError, auth::ApiAuthUser},
    models::{Phase, User},
};

// ============================================================
// 【教学：PhaseOut —— 阶段 DTO】
// ============================================================
// 比 models::Phase 多一个 days 字段（坚持天数，实时计算）。
// 为什么不在 models::Phase 里加？因为 days 不是数据库列，
// 是"展示层派生数据"——模型保持数据库镜像，DTO 负责加派生字段。
#[derive(Serialize)]
pub struct PhaseOut
{
    pub id: i64,
    pub name: String,
    pub note: String,
    pub start_date: Option<String>,
    pub archived: bool,
    /// 坚持天数（start_date 为空 → 0）
    pub days: i64,
}

// ============================================================
// 【教学：PhaseCreateReq —— 创建阶段请求体】
// ============================================================
// 页面层表单 start_date 用 String（空串 = 未设置），
// API 层客户端直接传 null 表示"未设置" → Option<String>。
// serde_json 里 null → None，缺字段 → None，语义一致。
#[derive(Deserialize)]
pub struct PhaseCreateReq
{
    pub name: String,
    pub note: String,
    #[serde(default)]
    pub start_date: Option<String>,
}

// ============================================================
// 【教学：坚持天数 —— 复用页面层同款 SQL】
// ============================================================
// 返回"今天 - start_date"的自然日差。
// start_date = None → 0（未设置开始日期）
async fn calc_days(pool: &SqlitePool, start_date: &Option<String>) -> Result<i64, ApiError>
{
    match start_date
    {
        Some(d) => sqlx::query_scalar::<_, i64>(
            "SELECT CAST(julianday('now','localtime') - julianday(?) AS INTEGER)",
        )
        .bind(d)
        .fetch_one(pool)
        .await
        .map_err(ApiError::Database),
        None => Ok(0),
    }
}

// ============================================================
// 【教学：Phase → PhaseOut 转换】
// ============================================================
// async fn 里没法用 sync 的 From（要 await 查天数），
// 所以用普通函数 phase_out，传 pool 进去。
async fn phase_out(pool: &SqlitePool, p: &Phase) -> Result<PhaseOut, ApiError>
{
    let days = calc_days(pool, &p.start_date).await?;
    Ok(PhaseOut {
        id: p.id,
        name: p.name.clone(),
        note: p.note.clone(),
        start_date: p.start_date.clone(),
        archived: p.archived,
        days,
    })
}

// ============================================================
// 阶段列表（GET /api/v1/phases）
// ============================================================
/// 阶段列表（进行中在前，已归档在后；含坚持天数）
///
/// 【教学：与页面 list 的差异】
/// 页面 list：查两次（active/archived 分两个区渲染）
/// API list  ：一次查全部，客户端自己分区（JSON 数组天然无分区概念）
/// 所以 API 用一条 SQL：ORDER BY archived ASC, created_at DESC
///   archived ASC：false(0) 在前，true(1) 在后 → 进行中先出现
///
/// 【实现步骤】
/// 1. 签名：State + ApiAuthUser
/// 2. SELECT * FROM phases WHERE user_id = ? ORDER BY archived ASC, created_at DESC
/// 3. 迭代器 map 转 PhaseOut（每个要查 days）
/// 4. collect Vec<PhaseOut> → Json
pub async fn list(
    State(state): State<AppState>,
    ApiAuthUser(user): ApiAuthUser,
) -> Result<Json<Vec<PhaseOut>>, ApiError>
{
    let pool = state.pool.read().await.clone();

    let phases = sqlx::query_as::<_, Phase>(
        "SELECT * FROM phases WHERE user_id = ? ORDER BY archived ASC, created_at DESC",
    )
    .bind(&user.id)
    .fetch_all(&pool)
    .await
    .map_err(ApiError::Database)?;

    // 【教学：map + collect —— 逐个转换】
    // phases.iter() 是借用迭代器，map 里调 async 函数要 .await
    // → 不能直接用同步 map，要用 stream/futures 或先 collect 再循环。
    // M8 教学简化：用 for 循环收集（直观、无新依赖）。
    let mut out = Vec::with_capacity(phases.len());
    for p in &phases
    {
        out.push(phase_out(&pool, p).await?);
    }

    Ok(Json(out))
}

// ============================================================
// 创建阶段（POST /api/v1/phases）
// ============================================================
/// 创建阶段 → 返回新阶段 JSON（含 id）
///
/// 【教学：与页面 create 的差异】
/// 页面 create：校验 → 查重 → INSERT → 302 重定向
/// API create ：校验 → 查重 → INSERT → 返回 JSON（客户端拿 id 继续操作）
///
/// 【教学：INSERT ... RETURNING id —— 拿回新 id】
/// 页面层用 execute() 不关心 id（重定向就好），
/// API 层要返回"创建成功的对象"（含 id），必须拿回自增 id。
/// SQLite 支持 RETURNING 子句（3.35+），一行搞定：
///   INSERT INTO phases ... RETURNING id → fetch_one → i64
///
/// 【实现步骤】
/// 1. 签名：State + ApiAuthUser + Json<PhaseCreateReq>
/// 2. 校验：name 非空（trim().is_empty() → Validation）
/// 3. 查重：SELECT id FROM phases WHERE user_id = ? AND name = ?
///    → fetch_optional → Some → Err(Validation("阶段名已存在"))
/// 4. 转换 start_date：空串 → None（API 客户端可能传 ""）
/// 5. INSERT ... RETURNING id → fetch_one
/// 6. 按新 id 查完整 Phase → phase_out → Json
pub async fn create(
    State(state): State<AppState>,
    ApiAuthUser(user): ApiAuthUser,
    Json(req): Json<PhaseCreateReq>,
) -> Result<Json<PhaseOut>, ApiError>
{
    let pool = state.pool.read().await.clone();

    // 校验：name 非空
    if req.name.trim().is_empty()
    {
        return Err(ApiError::Validation("阶段名称不能为空".to_string()));
    }

    // 查重（数据隔离 + 防重名）
    if sqlx::query_scalar::<_, i64>("SELECT id FROM phases WHERE user_id = ? AND name = ?")
        .bind(&user.id)
        .bind(&req.name)
        .fetch_optional(&pool)
        .await
        .map_err(ApiError::Database)?
        .is_some()
    {
        return Err(ApiError::Validation("阶段名已存在".to_string()));
    }

    // 转换 start_date：空串 → None（和页面表单一致）
    let start_date = match req.start_date.as_deref()
    {
        Some(s) if !s.trim().is_empty() => Some(s.trim().to_string()),
        _ => None,
    };

    // INSERT + RETURNING id（拿回新 id）
    let new_id = sqlx::query_scalar::<_, i64>(
        "INSERT INTO phases (user_id, name, note, start_date) VALUES (?, ?, ?, ?)
        RETURNING id",
    )
    .bind(&user.id)
    .bind(&req.name)
    .bind(&req.note)
    .bind(&start_date)
    .fetch_one(&pool)
    .await
    .map_err(ApiError::Database)?;

    // 查完整行 → 转 DTO → 返回
    let phase = sqlx::query_as::<_, Phase>("SELECT * FROM phases WHERE id = ? AND user_id = ?")
        .bind(&new_id)
        .bind(&user.id)
        .fetch_one(&pool)
        .await
        .map_err(ApiError::Database)?;

    Ok(Json(phase_out(&pool, &phase).await?))
}

// ============================================================
// 阶段详情（GET /api/v1/phases/{id}）
// ============================================================
/// 阶段详情（含坚持天数）
///
/// 【教学：fetch_optional → ok_or_else 404 模式】
/// 查不到（不存在或不属于当前用户）→ Err(NotFound)。
/// 这是"按 id 查询"的标准三连：
///   query_as → fetch_optional → ok_or_else(NotFound)
///
/// 【实现步骤】
/// 1. 签名：State + ApiAuthUser + Path(id)
/// 2. SELECT * FROM phases WHERE id = ? AND user_id = ? → fetch_optional
/// 3. None → Err(NotFound)；Some → phase_out → Json
pub async fn detail(
    State(state): State<AppState>,
    ApiAuthUser(user): ApiAuthUser,
    Path(id): Path<i64>,
) -> Result<Json<PhaseOut>, ApiError>
{
    let pool = state.pool.read().await.clone();

    let phase = sqlx::query_as::<_, Phase>("SELECT * FROM phases WHERE id = ? AND user_id = ?")
        .bind(&id)
        .bind(&user.id)
        .fetch_optional(&pool)
        .await
        .map_err(ApiError::Database)?
        .ok_or_else(|| ApiError::NotFound("阶段不存在".to_string()))?;

    Ok(Json(phase_out(&pool, &phase).await?))
}

// ============================================================
// 【教学：PhaseUpdateReq —— PATCH 请求体】
// ============================================================
// PATCH 语义：部分更新（只传要改的字段）。
// serde 的 Option + #[serde(default)]：
//   - 缺字段 → None（不改）
//   - 传 null → None（不改）
//   - 传值    → Some（更新）
// ⚠️ 注意：这和"把字段设为 null"（清空 start_date）冲突——
// M8 教学简化：PATCH 不支持清空 start_date（传 null 视为不改）。
// 若需要清空，用 ""（空串）→ 转 None 存库。
#[derive(Deserialize)]
pub struct PhaseUpdateReq
{
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub note: Option<String>,
    #[serde(default)]
    pub start_date: Option<String>,
}

// ============================================================
// 更新阶段（PATCH /api/v1/phases/{id}）
// ============================================================
/// 更新阶段（部分更新，只改传了的字段）
///
/// 【教学：动态拼 SQL vs 全量更新】
/// PATCH 只更新传了的字段。两种做法：
///   a. 动态拼 SQL（根据 Option 判断加 SET 子句）——复杂、易注入
///   b. 全量更新（读旧值，没传的用旧值补上）——简单、安全
/// M8 用 b：先查旧行 → 合并请求字段 → UPDATE 全列。
///
/// 【实现步骤】
/// 1. 签名：State + ApiAuthUser + Path(id) + Json<PhaseUpdateReq>
/// 2. 查旧行（数据隔离）→ None → NotFound
/// 3. 合并：name = req.name.unwrap_or(old.name)，其余同理
/// 4. UPDATE phases SET name=?, note=?, start_date=? WHERE id = ? AND user_id = ?
///    （rows_affected() == 0 → NotFound，理论上不会发生）
/// 5. 查新行 → phase_out → Json
pub async fn update(
    State(state): State<AppState>,
    ApiAuthUser(user): ApiAuthUser,
    Path(id): Path<i64>,
    Json(req): Json<PhaseUpdateReq>,
) -> Result<Json<PhaseOut>, ApiError>
{
    let pool = state.pool.read().await.clone();

    // ① 查旧行（数据隔离）
    let old = sqlx::query_as::<_, Phase>("SELECT * FROM phases WHERE id = ? AND user_id = ?")
        .bind(&id)
        .bind(&user.id)
        .fetch_optional(&pool)
        .await
        .map_err(ApiError::Database)?
        .ok_or_else(|| ApiError::NotFound("阶段不存在".to_string()))?;

    // ② 合并（没传的用旧值）
    let name = req.name.unwrap_or(old.name);
    let note = req.note.unwrap_or(old.note);
    // start_date：请求传了非空 → 更新；传了空串 → None；没传 → 旧值
    let start_date = match req.start_date
    {
        Some(s) if !s.trim().is_empty() => Some(s.trim().to_string()),
        Some(_) => None,
        None => old.start_date,
    };

    // 校验 name 非空
    if name.trim().is_empty()
    {
        return Err(ApiError::Validation("阶段名称不能为空".to_string()));
    }

    // ③ 全量 UPDATE
    let ret = sqlx::query(
        "UPDATE phases SET name = ?, note = ?, start_date = ? WHERE id = ? AND user_id = ?",
    )
    .bind(&name)
    .bind(&note)
    .bind(&start_date)
    .bind(&id)
    .bind(&user.id)
    .execute(&pool)
    .await
    .map_err(ApiError::Database)?;

    if ret.rows_affected() == 0
    {
        return Err(ApiError::NotFound("阶段不存在".to_string()));
    }

    // ④ 查新行返回
    let phase = sqlx::query_as::<_, Phase>("SELECT * FROM phases WHERE id = ? AND user_id = ?")
        .bind(&id)
        .bind(&user.id)
        .fetch_one(&pool)
        .await
        .map_err(ApiError::Database)?;

    Ok(Json(phase_out(&pool, &phase).await?))
}

// ============================================================
// 归档（POST /api/v1/phases/{id}/archive）
// ============================================================
/// 归档阶段（archived = 1，只读）
///
/// 【教学：为什么阶段用"归档"而不是"删除"？】
/// 阶段是"时间容器"，计划/记录都挂在 phase_id 上，
/// 删了阶段 = 删历史。归档是软删除：数据保留，只是不可编辑。
///
/// 【实现步骤】
/// 1. 签名：State + ApiAuthUser + Path(id)
/// 2. UPDATE phases SET archived = 1 WHERE id = ? AND user_id = ?
/// 3. rows_affected() == 0 → NotFound
/// 4. 返回更新后的 PhaseOut
pub async fn archive(
    State(state): State<AppState>,
    ApiAuthUser(user): ApiAuthUser,
    Path(id): Path<i64>,
) -> Result<Json<PhaseOut>, ApiError>
{
    set_archived(state, user, id, true).await
}

// ============================================================
// 启用（POST /api/v1/phases/{id}/unarchive）
// ============================================================
/// 启用归档阶段（archived = 0，恢复可编辑）
pub async fn unarchive(
    State(state): State<AppState>,
    ApiAuthUser(user): ApiAuthUser,
    Path(id): Path<i64>,
) -> Result<Json<PhaseOut>, ApiError>
{
    set_archived(state, user, id, false).await
}

/// 归档/启用共用实现（DRY：两个 handler 只有 archived 值不同）
async fn set_archived(
    state: AppState,
    user: User,
    id: i64,
    archived: bool,
) -> Result<Json<PhaseOut>, ApiError>
{
    let pool = state.pool.read().await.clone();

    let ret = sqlx::query("UPDATE phases SET archived = ? WHERE id = ? AND user_id = ?")
        .bind(archived)
        .bind(&id)
        .bind(&user.id)
        .execute(&pool)
        .await
        .map_err(ApiError::Database)?;

    if ret.rows_affected() == 0
    {
        return Err(ApiError::NotFound("阶段不存在".to_string()));
    }

    let phase = sqlx::query_as::<_, Phase>("SELECT * FROM phases WHERE id = ? AND user_id = ?")
        .bind(&id)
        .bind(&user.id)
        .fetch_one(&pool)
        .await
        .map_err(ApiError::Database)?;

    Ok(Json(phase_out(&pool, &phase).await?))
}
