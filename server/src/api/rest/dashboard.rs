#[derive(serde::Serialize)]
pub struct DashboardOut
{
    pub phase_count: i64,
    pub exercise_count: i64,
    pub template_count: i64,
    pub plan_count: i64,
    pub current_phase: Option<DashboardPhase>,
}

#[derive(serde::Serialize)]
pub struct DashboardPhase
{
    pub name: String,
    pub days: i64,
}

pub async fn dashboard(
    axum::extract::State(state): axum::extract::State<crate::AppState>,
    crate::api::rest::auth::ApiAuthUser(user): crate::api::rest::auth::ApiAuthUser,
) -> Result<axum::Json<DashboardOut>, crate::api::rest::ApiError>
{
    let pool = state.pool.read().await.clone();
    let phase_count = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM phases WHERE user_id = ?")
        .bind(user.id)
        .fetch_one(&pool)
        .await
        .map_err(crate::api::rest::ApiError::Database)?;
    let exercise_count =
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM exercises WHERE user_id = ?")
            .bind(user.id)
            .fetch_one(&pool)
            .await
            .map_err(crate::api::rest::ApiError::Database)?;
    let template_count = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM templates t INNER JOIN phases p ON t.phase_id = p.id
         WHERE p.user_id = ?",
    )
    .bind(user.id)
    .fetch_one(&pool)
    .await
    .map_err(crate::api::rest::ApiError::Database)?;
    let plan_count = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM plans pl INNER JOIN phases p ON pl.phase_id = p.id
         WHERE p.user_id = ?",
    )
    .bind(user.id)
    .fetch_one(&pool)
    .await
    .map_err(crate::api::rest::ApiError::Database)?;
    let current = sqlx::query_as::<_, crate::models::Phase>(
        "SELECT * FROM phases WHERE user_id = ? AND archived = 0
         ORDER BY created_at DESC LIMIT 1",
    )
    .bind(user.id)
    .fetch_optional(&pool)
    .await
    .map_err(crate::api::rest::ApiError::Database)?;
    let current_phase = match current
    {
        Some(phase) =>
        {
            let days = match phase.start_date.as_deref()
            {
                Some(start) => sqlx::query_scalar::<_, i64>(
                    "SELECT CAST(julianday('now','localtime') - julianday(?) AS INTEGER)",
                )
                .bind(start)
                .fetch_one(&pool)
                .await
                .map_err(crate::api::rest::ApiError::Database)?,
                None => 0,
            };
            Some(DashboardPhase {
                name: phase.name,
                days,
            })
        },
        None => None,
    };
    Ok(axum::Json(DashboardOut {
        phase_count,
        exercise_count,
        template_count,
        plan_count,
        current_phase,
    }))
}
