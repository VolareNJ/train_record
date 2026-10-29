#[derive(Debug, Clone, serde::Deserialize)]
pub struct User
{
    pub id: i64,
    pub username: String,
    pub is_admin: bool,
    pub body_weight: Option<f64>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct LoginResponse
{
    pub user: User,
    #[serde(default)]
    pub token: String,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct Phase
{
    pub id: i64,
    pub name: String,
    pub note: String,
    pub start_date: Option<String>,
    pub archived: bool,
    pub days: i64,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct Exercise
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
    pub last_record_date: Option<String>,
    pub best_1rm: Option<f64>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct TemplateItem
{
    pub exercise_id: i64,
    pub exercise_name: String,
    pub plan_sets: Option<i64>,
    pub plan_reps: Option<i64>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct Template
{
    pub id: i64,
    pub name: String,
    pub items: Vec<TemplateItem>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct PlanItem
{
    pub exercise_id: i64,
    pub exercise_name: String,
    pub plan_sets: Option<i64>,
    pub plan_reps: Option<i64>,
    pub plan_weight: Option<f64>,
    pub plan_rest: Option<i64>,
    pub plan_key_points: Option<String>,
    pub plan_note: Option<String>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct Plan
{
    pub id: i64,
    pub date: String,
    pub note: String,
    pub items: Vec<PlanItem>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct LastRecord
{
    pub id: i64,
    pub weight: f64,
    pub sets: i64,
    pub reps: i64,
    pub rest: i64,
    pub feeling: String,
    pub strategy: String,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct TodayItem
{
    pub id: i64,
    pub exercise_name: String,
    pub body_part: String,
    pub plan_sets: Option<i64>,
    pub plan_reps: Option<i64>,
    pub plan_weight: Option<f64>,
    pub plan_rest: Option<i64>,
    pub plan_key_points: Option<String>,
    pub last_record: Option<LastRecord>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct TodayPlan
{
    pub id: i64,
    pub note: String,
    pub items: Vec<TodayItem>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct TodayPhase
{
    pub id: i64,
    pub name: String,
    pub days: i64,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct Today
{
    pub phase: Option<TodayPhase>,
    pub date: String,
    pub plan: Option<TodayPlan>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct Calendar
{
    pub train_days: Vec<String>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct DayRecord
{
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
    #[serde(rename = "1rm")]
    pub one_rm: f64,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct ExerciseBrief
{
    pub name: String,
    pub body_part: String,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct ExerciseRecord
{
    pub date: String,
    pub weight: f64,
    pub sets: i64,
    pub reps: i64,
    #[serde(rename = "1rm")]
    pub one_rm: f64,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct ExerciseStats
{
    pub exercise: ExerciseBrief,
    pub records: Vec<ExerciseRecord>,
    pub best_1rm: f64,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct PhaseInput
{
    pub name: String,
    pub note: String,
    pub start_date: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ExerciseInput
{
    pub name: String,
    pub body_part: String,
    pub default_mode: String,
    pub bar_weight: f64,
    pub default_unit: String,
    pub default_sets: i64,
    pub default_reps: i64,
    pub key_points: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct TemplateItemInput
{
    pub exercise_id: i64,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct TemplateInput
{
    pub name: String,
    pub items: Vec<TemplateItemInput>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct PlanItemInput
{
    pub exercise_id: i64,
    pub plan_sets: Option<i64>,
    pub plan_reps: Option<i64>,
    pub plan_weight: Option<f64>,
    pub plan_rest: Option<i64>,
    pub plan_key_points: Option<String>,
    pub plan_note: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct PlanInput
{
    pub date: String,
    pub note: String,
    pub items: Vec<PlanItemInput>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct RecordInput
{
    pub weight: f64,
    pub sets: i64,
    pub reps: i64,
    pub rest: i64,
    pub feeling: String,
    pub strategy: String,
    pub key_points: String,
    pub completed: bool,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct MeInput
{
    pub body_weight: Option<f64>,
}
