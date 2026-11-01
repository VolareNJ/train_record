use axum::response::IntoResponse;

// 管理员 API：用户管理 + 数据备份/导出。
// 这些能力原先只存在于 SSR 页面（handlers/auth.rs / handlers/backup.rs），
// 这里补一套 REST 出口，供 iced 客户端在应用内直接使用。

#[derive(serde::Serialize)]
pub struct AdminUserOut
{
    pub id: i64,
    pub username: String,
    pub is_admin: bool,
}

#[derive(serde::Deserialize)]
pub struct CreateUserReq
{
    pub username: String,
    pub password: String,
    #[serde(default)]
    pub is_admin: bool,
}

#[derive(serde::Deserialize)]
pub struct ExportQuery
{
    pub format: Option<String>,
}

fn require_admin(user: &crate::models::User) -> Result<(), crate::api::rest::ApiError>
{
    if user.is_admin
    {
        Ok(())
    }
    else
    {
        Err(crate::api::rest::ApiError::Forbidden(
            "需要管理员权限".to_string(),
        ))
    }
}

pub async fn users(
    axum::extract::State(state): axum::extract::State<crate::AppState>,
    crate::api::rest::auth::ApiAuthUser(user): crate::api::rest::auth::ApiAuthUser,
) -> Result<axum::Json<Vec<AdminUserOut>>, crate::api::rest::ApiError>
{
    require_admin(&user)?;
    let pool = state.pool.read().await.clone();
    let users = sqlx::query_as::<_, crate::models::User>("SELECT * FROM users ORDER BY id ASC")
        .fetch_all(&pool)
        .await
        .map_err(crate::api::rest::ApiError::Database)?;
    Ok(axum::Json(
        users
            .into_iter()
            .map(|user| AdminUserOut {
                id: user.id,
                username: user.username,
                is_admin: user.is_admin,
            })
            .collect(),
    ))
}

pub async fn create_user(
    axum::extract::State(state): axum::extract::State<crate::AppState>,
    crate::api::rest::auth::ApiAuthUser(user): crate::api::rest::auth::ApiAuthUser,
    axum::Json(req): axum::Json<CreateUserReq>,
) -> Result<axum::Json<AdminUserOut>, crate::api::rest::ApiError>
{
    require_admin(&user)?;
    if req.username.trim().is_empty() || req.password.len() < 6
    {
        return Err(crate::api::rest::ApiError::Validation(
            "用户名不能为空，密码至少 6 位".to_string(),
        ));
    }
    let pool = state.pool.read().await.clone();
    let password_hash = crate::auth::hash_password(&req.password)
        .map_err(|_| crate::api::rest::ApiError::Other("密码哈希失败".to_string()))?;
    let id = sqlx::query_scalar::<_, i64>(
        "INSERT INTO users (username, password_hash, is_admin) VALUES (?, ?, ?) RETURNING id",
    )
    .bind(req.username.trim())
    .bind(password_hash)
    .bind(req.is_admin)
    .fetch_one(&pool)
    .await
    .map_err(crate::api::rest::ApiError::Database)?;
    Ok(axum::Json(AdminUserOut {
        id,
        username: req.username.trim().to_string(),
        is_admin: req.is_admin,
    }))
}

pub async fn backup_download(
    axum::extract::State(state): axum::extract::State<crate::AppState>,
    crate::api::rest::auth::ApiAuthUser(user): crate::api::rest::auth::ApiAuthUser,
) -> Result<axum::response::Response, crate::api::rest::ApiError>
{
    require_admin(&user)?;
    let pool = state.pool.read().await.clone();
    let today = sqlx::query_scalar::<_, String>("SELECT date('now', 'localtime')")
        .fetch_one(&pool)
        .await
        .map_err(crate::api::rest::ApiError::Database)?;
    let bytes = tokio::fs::read(&state.config.database_path)
        .await
        .map_err(|error| crate::api::rest::ApiError::Other(error.to_string()))?;
    if !bytes.starts_with(b"SQLite format 3\0")
    {
        return Err(crate::api::rest::ApiError::Validation(
            "数据库文件格式不正确".to_string(),
        ));
    }
    let mut headers = axum::http::HeaderMap::new();
    headers.insert(
        axum::http::header::CONTENT_DISPOSITION,
        format!("attachment; filename=\"train_record_{today}.db\"")
            .parse()
            .map_err(|_| crate::api::rest::ApiError::Other("响应头构造失败".to_string()))?,
    );
    headers.insert(
        axum::http::header::CONTENT_TYPE,
        "application/octet-stream"
            .parse()
            .map_err(|_| crate::api::rest::ApiError::Other("响应头构造失败".to_string()))?,
    );
    Ok((headers, bytes).into_response())
}

pub async fn backup_upload(
    axum::extract::State(state): axum::extract::State<crate::AppState>,
    crate::api::rest::auth::ApiAuthUser(user): crate::api::rest::auth::ApiAuthUser,
    mut multipart: axum::extract::Multipart,
) -> Result<axum::Json<serde_json::Value>, crate::api::rest::ApiError>
{
    require_admin(&user)?;
    let field = loop
    {
        match multipart
            .next_field()
            .await
            .map_err(|error| crate::api::rest::ApiError::Other(error.to_string()))?
        {
            Some(field) if field.name() == Some("db_file") => break field,
            Some(_) => continue,
            None =>
            {
                return Err(crate::api::rest::ApiError::Validation(
                    "没有收到数据库文件".to_string(),
                ));
            },
        }
    };
    let filename = field.file_name().unwrap_or("").to_string();
    if !filename.to_ascii_lowercase().ends_with(".db")
    {
        return Err(crate::api::rest::ApiError::Validation(
            "只支持 .db 文件".to_string(),
        ));
    }
    let bytes = field
        .bytes()
        .await
        .map_err(|error| crate::api::rest::ApiError::Other(error.to_string()))?;
    if !bytes.starts_with(b"SQLite format 3\0")
    {
        return Err(crate::api::rest::ApiError::Validation(
            "不是有效的 SQLite 数据库".to_string(),
        ));
    }

    let pool = state.pool.read().await.clone();
    let now =
        sqlx::query_scalar::<_, String>("SELECT strftime('%Y%m%d-%H%M%S', 'now', 'localtime')")
            .fetch_one(&pool)
            .await
            .map_err(crate::api::rest::ApiError::Database)?;
    let db_path = state.config.database_path.clone();
    let parent = std::path::Path::new(&db_path)
        .parent()
        .map(|path| path.to_string_lossy().into_owned())
        .unwrap_or_else(|| ".".to_string());
    let backup_dir = format!("{parent}/backup");
    tokio::fs::create_dir_all(&backup_dir)
        .await
        .map_err(|error| crate::api::rest::ApiError::Other(error.to_string()))?;
    let file_name = std::path::Path::new(&db_path)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("train_record.db");
    tokio::fs::rename(&db_path, format!("{backup_dir}/{file_name}.bak-{now}"))
        .await
        .map_err(|error| crate::api::rest::ApiError::Other(error.to_string()))?;

    let mut pool_guard = state.pool.write().await;
    pool_guard.close().await;
    tokio::fs::write(&db_path, bytes)
        .await
        .map_err(|error| crate::api::rest::ApiError::Other(error.to_string()))?;
    *pool_guard = sqlx::SqlitePool::connect(&db_path)
        .await
        .map_err(crate::api::rest::ApiError::Database)?;
    Ok(axum::Json(serde_json::json!({ "ok": true })))
}

pub async fn export_records(
    axum::extract::State(state): axum::extract::State<crate::AppState>,
    crate::api::rest::auth::ApiAuthUser(user): crate::api::rest::auth::ApiAuthUser,
    axum::extract::Query(query): axum::extract::Query<ExportQuery>,
) -> Result<axum::response::Response, crate::api::rest::ApiError>
{
    require_admin(&user)?;
    let pool = state.pool.read().await.clone();
    let records = sqlx::query_as::<_, crate::models::Record>(
        "SELECT r.* FROM records r INNER JOIN exercises e ON r.exercise_id = e.id
         WHERE e.user_id = ? ORDER BY r.record_date DESC, r.id",
    )
    .bind(user.id)
    .fetch_all(&pool)
    .await
    .map_err(crate::api::rest::ApiError::Database)?;
    let names: std::collections::HashMap<i64, String> =
        sqlx::query_as::<_, crate::models::Exercise>("SELECT * FROM exercises WHERE user_id = ?")
            .bind(user.id)
            .fetch_all(&pool)
            .await
            .map_err(crate::api::rest::ApiError::Database)?
            .into_iter()
            .map(|exercise| (exercise.id, exercise.name))
            .collect();
    let today = sqlx::query_scalar::<_, String>("SELECT date('now', 'localtime')")
        .fetch_one(&pool)
        .await
        .map_err(crate::api::rest::ApiError::Database)?;
    let format = query.format.as_deref().unwrap_or("csv");
    let (mime, body) = match format
    {
        "csv" =>
        {
            let header = "exercise_id,name,record_date,completed,weight,sets,reps,rest,feeling,strategy,key_points";
            let rows = records
                .iter()
                .map(|record| {
                    [
                        escape_csv(&record.exercise_id.to_string()),
                        escape_csv(
                            names
                                .get(&record.exercise_id)
                                .map(String::as_str)
                                .unwrap_or(""),
                        ),
                        escape_csv(&record.record_date),
                        escape_csv(&record.completed.to_string()),
                        escape_csv(&record.weight.to_string()),
                        escape_csv(&record.sets.to_string()),
                        escape_csv(&record.reps.to_string()),
                        escape_csv(&record.rest.to_string()),
                        escape_csv(&record.feeling),
                        escape_csv(&record.strategy),
                        escape_csv(&record.key_points),
                    ]
                    .join(",")
                })
                .collect::<Vec<_>>()
                .join("\n");
            ("text/csv; charset=utf-8", format!("{header}\n{rows}"))
        },
        "json" =>
        {
            let rows = records
                .iter()
                .map(|record| {
                    serde_json::json!({
                        "exercise_id": record.exercise_id,
                        "name": names.get(&record.exercise_id).map(String::as_str).unwrap_or(""),
                        "record_date": record.record_date,
                        "completed": record.completed,
                        "weight": record.weight,
                        "sets": record.sets,
                        "reps": record.reps,
                        "rest": record.rest,
                        "feeling": record.feeling,
                        "strategy": record.strategy,
                        "key_points": record.key_points,
                    })
                })
                .collect::<Vec<_>>();
            (
                "application/json",
                serde_json::to_string(&rows)
                    .map_err(|error| crate::api::rest::ApiError::Other(error.to_string()))?,
            )
        },
        other =>
        {
            return Err(crate::api::rest::ApiError::Validation(format!(
                "format 只支持 csv/json，收到: {other}"
            )));
        },
    };
    let mut headers = axum::http::HeaderMap::new();
    headers.insert(
        axum::http::header::CONTENT_DISPOSITION,
        format!("attachment; filename=\"records_{today}.{format}\"")
            .parse()
            .map_err(|_| crate::api::rest::ApiError::Other("响应头构造失败".to_string()))?,
    );
    headers.insert(
        axum::http::header::CONTENT_TYPE,
        mime.parse()
            .map_err(|_| crate::api::rest::ApiError::Other("响应头构造失败".to_string()))?,
    );
    Ok((headers, body).into_response())
}

fn escape_csv(value: &str) -> String
{
    if value.contains(',') || value.contains('"') || value.contains('\n')
    {
        format!("\"{}\"", value.replace('"', "\"\""))
    }
    else
    {
        value.to_string()
    }
}
