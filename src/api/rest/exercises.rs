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
//  阶段要求：M8 你来实现本文件所有函数。
//   完整实现已备份在 docs/learning_path/M8_ref/，实现完成后对照检查。
// ============================================================

//  注意（M9 全路径约定）：实现 exercise_out 时直接用全路径，本文件不需要 import：
//     crate::calc::epley_1rm(...)
//     sqlx::query_as::<_, crate::models::Record>(...) / crate::models::Exercise

// ============================================================
// 【教学：ExerciseOut —— 动作 DTO】
// ============================================================
// 比 models::Exercise 多一个 last_record 摘要（最近一次训练）：
//   date：最近训练日期
//   best_1rm：该动作历史最高 1RM（实时计算，不落库）
// 为什么"最近记录"不在 models 里？它是派生数据，查询时算。
#[derive(serde::Serialize)]
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
#[derive(serde::Deserialize)]
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

pub(crate) fn default_mode() -> String
{
    "bar".to_string()
}
pub(crate) fn default_bar_weight() -> f64
{
    20.0
}
pub(crate) fn default_unit() -> String
{
    "kg".to_string()
}
pub(crate) fn default_sets() -> i64
{
    3
}
pub(crate) fn default_reps() -> i64
{
    8
}

// ============================================================
// 【教学：Exercise → ExerciseOut 转换】
// ============================================================
// 派生数据（last_record_date / best_1rm）要查 records 表，
// 所以是 async 函数（不能 From）。
async fn exercise_out(
    pool: &sqlx::SqlitePool,
    ex: &crate::models::Exercise,
) -> Result<ExerciseOut, crate::api::rest::ApiError>
{
    // 1. 查该动作全部记录（升序）：last() 即最近一次训练
    let records = sqlx::query_as::<_, crate::models::Record>(
        "SELECT * FROM records WHERE exercise_id = ? ORDER BY record_date ASC, id ASC",
    )
    .bind(&ex.id)
    .fetch_all(pool)
    .await
    .map_err(crate::api::rest::ApiError::Database)?;

    // 2. last_record_date：升序 → 最后一条 = 最近一次训练
    //     用 map 不用 unwrap：一条记录都没有 → None
    //    （"从未练过"是合法状态，unwrap 空迭代器会 panic！）
    let last_record_date = records.last().map(|r| r.record_date.clone());

    // 3. best_1rm：历史最高 1RM（实时计算，不落库）
    //    三步链式流水线（函数式风格）：
    //      map   → 每条记录算一个 1RM（epley_1rm）
    //      fold  → 把一串 1RM 折叠成最大值（见 pipe 下方注释）
    //      pipe  → 把 "0.0"（无效输入/无记录）清理成 None（见下方注释）
    let best_1rm = records
        .iter()
        .map(|r| crate::calc::epley_1rm(r.weight, r.reps))
        .fold(0.0_f64, f64::max)
        .pipe(|v| if v > 0.0 { Some(v) } else { None });

    // 4. 组装 ExerciseOut：基础字段照抄 ex（ex 是借用，String 要 clone），
    //    派生字段用上面算好的两个值
    Ok(ExerciseOut {
        id: ex.id,
        name: ex.name.clone(),
        body_part: ex.body_part.clone(),
        default_mode: ex.default_mode.clone(),
        bar_weight: ex.bar_weight,
        default_unit: ex.default_unit.clone(),
        default_sets: ex.default_sets,
        default_reps: ex.default_reps,
        key_points: ex.key_points.clone(),
        last_record_date,
        best_1rm,
    })
}

// 【教学：fold —— 迭代器的"折叠"（把一串值压成一个值）】
// fold(初始值, 闭包) 遍历迭代器，闭包每次接收"累积值 + 当前元素"，
// 返回新的累积值，最后只剩一个值：
//   records.iter().map(1RM).fold(0.0, f64::max)
//     = 0.0 与第 1 个 1RM 取大 → 与第 2 个取大 → ... → 与最后 1 个取大
//     = 历史最高 1RM
// 为什么初始值用 0.0？
//   - 空记录时结果就是 0.0（迭代器为空也不会 panic）
//   - 1RM 不可能是负数，0.0 是安全的"中性起点"
// 对比：也可用 .max_by()，但空迭代器返回 None 还要再处理；
// fold 给出一个"总有结果"的确定值，适合继续链下去。
//
// 【教学：pipe —— 把值喂给闭包（标准库没有，局部 trait 替代）】
// v.pipe(f) 就是 f(v) 的链式写法，让"折叠结果"能继续接在链上：
//   .fold(...).pipe(|v| if v > 0.0 { Some(v) } else { None })
//   等价于拆开写：
//     let max = fold(...);                      // f64
//     if max > 0.0 { Some(max) } else { None }  // Option<f64>
// 这就是管道思想（JavaScript 的 |>、Elixir 的 |>、F# 的 |> 同一概念）：
// 数据从左到右流经一个个变换，不用起中间变量名。
//  只在"变换链中途"需要时用；两步以上才值得，别为单步引 trait。
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
#[derive(serde::Deserialize)]
pub struct ListQuery
{
    pub body_part: Option<String>,
}

pub async fn list(
    axum::extract::State(state): axum::extract::State<crate::AppState>,
    crate::api::rest::auth::ApiAuthUser(user): crate::api::rest::auth::ApiAuthUser,
    axum::extract::Query(query): axum::extract::Query<ListQuery>,
) -> Result<axum::Json<Vec<ExerciseOut>>, crate::api::rest::ApiError>
{
    let pool = state.pool.read().await.clone();
    // 【M9】协议无关逻辑抽到下方 exercise_list（gRPC 服务复用同一份 SQL）
    Ok(axum::Json(
        exercise_list(&pool, user.id, query.body_part.as_deref()).await?,
    ))
}

/// 协议无关实现（REST handler 与 M9 gRPC 服务共用）
pub(crate) async fn exercise_list(
    pool: &sqlx::SqlitePool,
    user_id: i64,
    body_part: Option<&str>,
) -> Result<Vec<ExerciseOut>, crate::api::rest::ApiError>
{
    // 空串筛选 = 不筛选（页面层同款）
    let part_filter = body_part.filter(|p| !p.is_empty());

    let exercises = match part_filter
    {
        None => sqlx::query_as::<_, crate::models::Exercise>(
            "SELECT * FROM exercises WHERE user_id = ? ORDER BY body_part, sort_order, id",
        )
        .bind(&user_id)
        .fetch_all(pool),
        Some(pt) => sqlx::query_as::<_, crate::models::Exercise>(
            "SELECT * FROM exercises WHERE user_id = ? AND body_part = ? ORDER BY sort_order, id",
        )
        .bind(&user_id)
        .bind(pt)
        .fetch_all(pool),
    }
    .await
    .map_err(crate::api::rest::ApiError::Database)?;

    let mut out = Vec::with_capacity(exercises.len());
    for ex in &exercises
    {
        out.push(exercise_out(pool, ex).await?);
    }

    Ok(out)
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
    axum::extract::State(state): axum::extract::State<crate::AppState>,
    crate::api::rest::auth::ApiAuthUser(user): crate::api::rest::auth::ApiAuthUser,
    axum::Json(req): axum::Json<ExerciseCreateReq>,
) -> Result<axum::Json<ExerciseOut>, crate::api::rest::ApiError>
{
    let pool = state.pool.read().await.clone();
    // 【M9】协议无关逻辑抽到下方 exercise_create（gRPC 服务复用同一份 SQL）
    Ok(axum::Json(exercise_create(&pool, user.id, &req).await?))
}

/// 协议无关实现（REST handler 与 M9 gRPC 服务共用）
pub(crate) async fn exercise_create(
    pool: &sqlx::SqlitePool,
    user_id: i64,
    req: &ExerciseCreateReq,
) -> Result<ExerciseOut, crate::api::rest::ApiError>
{
    if req.name.trim().is_empty() || req.body_part.trim().is_empty()
    {
        return Err(crate::api::rest::ApiError::Validation(
            "动作名和部位不能为空".to_string(),
        ));
    }

    // 查重（数据隔离 + 防重名，和页面 create 同款）
    if sqlx::query_scalar::<_, i64>("SELECT id FROM exercises WHERE user_id = ? AND name = ?")
        .bind(&user_id)
        .bind(&req.name)
        .fetch_optional(pool)
        .await
        .map_err(crate::api::rest::ApiError::Database)?
        .is_some()
    {
        return Err(crate::api::rest::ApiError::Validation(
            "动作名已存在".to_string(),
        ));
    }

    let new_id = sqlx::query_scalar::<_, i64>(
        "INSERT INTO exercises
        (user_id, name, body_part, default_mode, bar_weight, default_unit,
         default_sets, default_reps, key_points, sort_order)
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, 0)
        RETURNING id",
    )
    .bind(&user_id)
    .bind(&req.name)
    .bind(&req.body_part)
    .bind(&req.default_mode)
    .bind(&req.bar_weight)
    .bind(&req.default_unit)
    .bind(&req.default_sets)
    .bind(&req.default_reps)
    .bind(&req.key_points)
    .fetch_one(pool)
    .await
    .map_err(crate::api::rest::ApiError::Database)?;

    let ex = sqlx::query_as::<_, crate::models::Exercise>(
        "SELECT * FROM exercises WHERE id = ? AND user_id = ?",
    )
    .bind(&new_id)
    .bind(&user_id)
    .fetch_one(pool)
    .await
    .map_err(crate::api::rest::ApiError::Database)?;

    Ok(exercise_out(pool, &ex).await?)
}

// ============================================================
// 【教学：ExerciseUpdateReq —— PATCH 请求体】
// ============================================================
// 部分更新：缺字段 → 用旧值。数字字段 Option<f64>（null 视为不改）。
#[derive(serde::Deserialize)]
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
    axum::extract::State(state): axum::extract::State<crate::AppState>,
    crate::api::rest::auth::ApiAuthUser(user): crate::api::rest::auth::ApiAuthUser,
    axum::extract::Path(id): axum::extract::Path<i64>,
) -> Result<axum::Json<ExerciseOut>, crate::api::rest::ApiError>
{
    let pool = state.pool.read().await.clone();
    // 【M9】协议无关逻辑抽到下方 exercise_detail（gRPC 服务复用同一份 SQL）
    Ok(axum::Json(exercise_detail(&pool, user.id, id).await?))
}

/// 协议无关实现（REST handler 与 M9 gRPC 服务共用）
pub(crate) async fn exercise_detail(
    pool: &sqlx::SqlitePool,
    user_id: i64,
    exercise_id: i64,
) -> Result<ExerciseOut, crate::api::rest::ApiError>
{
    let ex = sqlx::query_as::<_, crate::models::Exercise>(
        "SELECT * FROM exercises WHERE id = ? AND user_id = ?",
    )
    .bind(&exercise_id)
    .bind(&user_id)
    .fetch_optional(pool)
    .await
    .map_err(crate::api::rest::ApiError::Database)?
    .ok_or_else(|| crate::api::rest::ApiError::NotFound("动作不存在".to_string()))?;

    Ok(exercise_out(pool, &ex).await?)
}

// ============================================================
// 更新动作（PATCH /api/v1/exercises/{id}）
// ============================================================
/// 更新动作（部分更新，先查旧值合并）
pub async fn update(
    axum::extract::State(state): axum::extract::State<crate::AppState>,
    crate::api::rest::auth::ApiAuthUser(user): crate::api::rest::auth::ApiAuthUser,
    axum::extract::Path(id): axum::extract::Path<i64>,
    axum::Json(req): axum::Json<ExerciseUpdateReq>,
) -> Result<axum::Json<ExerciseOut>, crate::api::rest::ApiError>
{
    let pool = state.pool.read().await.clone();
    // 【M9】协议无关逻辑抽到下方 exercise_update（gRPC 服务复用同一份 SQL）
    Ok(axum::Json(exercise_update(&pool, user.id, id, &req).await?))
}

/// 协议无关实现（REST handler 与 M9 gRPC 服务共用）
pub(crate) async fn exercise_update(
    pool: &sqlx::SqlitePool,
    user_id: i64,
    exercise_id: i64,
    req: &ExerciseUpdateReq,
) -> Result<ExerciseOut, crate::api::rest::ApiError>
{
    let old = sqlx::query_as::<_, crate::models::Exercise>(
        "SELECT * FROM exercises WHERE id = ? AND user_id = ?",
    )
    .bind(&exercise_id)
    .bind(&user_id)
    .fetch_optional(pool)
    .await
    .map_err(crate::api::rest::ApiError::Database)?
    .ok_or_else(|| crate::api::rest::ApiError::NotFound("动作不存在".to_string()))?;

    let name = req.name.clone().unwrap_or(old.name);
    let body_part = req.body_part.clone().unwrap_or(old.body_part);
    let default_mode = req.default_mode.clone().unwrap_or(old.default_mode);
    let bar_weight = req.bar_weight.unwrap_or(old.bar_weight);
    let default_unit = req.default_unit.clone().unwrap_or(old.default_unit);
    let default_sets = req.default_sets.unwrap_or(old.default_sets);
    let default_reps = req.default_reps.unwrap_or(old.default_reps);
    let key_points = req.key_points.clone().unwrap_or(old.key_points);

    if name.trim().is_empty() || body_part.trim().is_empty()
    {
        return Err(crate::api::rest::ApiError::Validation(
            "动作名和部位不能为空".to_string(),
        ));
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
    .bind(&exercise_id)
    .bind(&user_id)
    .execute(pool)
    .await
    .map_err(crate::api::rest::ApiError::Database)?;

    if ret.rows_affected() == 0
    {
        return Err(crate::api::rest::ApiError::NotFound(
            "动作不存在".to_string(),
        ));
    }

    let ex = sqlx::query_as::<_, crate::models::Exercise>(
        "SELECT * FROM exercises WHERE id = ? AND user_id = ?",
    )
    .bind(&exercise_id)
    .bind(&user_id)
    .fetch_one(pool)
    .await
    .map_err(crate::api::rest::ApiError::Database)?;

    Ok(exercise_out(pool, &ex).await?)
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
    axum::extract::State(state): axum::extract::State<crate::AppState>,
    crate::api::rest::auth::ApiAuthUser(user): crate::api::rest::auth::ApiAuthUser,
    axum::extract::Path(id): axum::extract::Path<i64>,
) -> Result<axum::Json<serde_json::Value>, crate::api::rest::ApiError>
{
    let pool = state.pool.read().await.clone();
    // 【M9】协议无关逻辑抽到下方 exercise_delete（gRPC 服务复用同一份 SQL）
    exercise_delete(&pool, user.id, id).await?;
    Ok(axum::Json(serde_json::json!({ "ok": true })))
}

/// 协议无关实现（REST handler 与 M9 gRPC 服务共用）
pub(crate) async fn exercise_delete(
    pool: &sqlx::SqlitePool,
    user_id: i64,
    exercise_id: i64,
) -> Result<(), crate::api::rest::ApiError>
{
    let ret = sqlx::query("DELETE FROM exercises WHERE id = ? AND user_id = ?")
        .bind(&exercise_id)
        .bind(&user_id)
        .execute(pool)
        .await
        .map_err(crate::api::rest::ApiError::Database)?;

    if ret.rows_affected() == 0
    {
        return Err(crate::api::rest::ApiError::NotFound(
            "动作不存在".to_string(),
        ));
    }

    Ok(())
}
