// ============================================================
// api/grpc/service/exercises.rs —— ExerciseService 实现（M9 第 6 步③）
// ============================================================
// 【教学：本文件两个重点】
//   1. 前 5 个一元方法：照抄 phases.rs 的三步骨架（读作参考，不挖空）
//   2. 第 6 个 `stream_exercise_series`：**服务器流式**（★ 挖空练习）
//      —— 这是 gRPC 相对 REST 的第一个硬能力，值得慢慢看注释。
//
// 【教学：DeleteExercise 为什么用 Ack 而不是"返回被删的 Exercise"？】
// 契约设计的一个常见取舍：
//   · 返回被删对象：客户端能确认"我删的是这个"（但要多查一次库/多传一份数据）
//   · 返回 Ack：省事，靠 NOT_FOUND 状态码表达"没删到"
// 本项目与 REST 一致（REST 也返回 {"ok": true}）。真要在客户端做"撤销删除"，
// 那就该返回完整对象——需求驱动契约，而不是反过来。
// ============================================================

use tonic::{Request, Response, Status};

use super::pool_of;
use crate::{
    AppState,
    api::{
        grpc::{auth, convert, pb},
        rest::{exercises as rest_exercises, stats as rest_stats},
    },
};

#[derive(Clone)]
pub struct ExerciseServiceImpl
{
    pub(crate) state: AppState,
}

impl ExerciseServiceImpl
{
    pub fn new(state: AppState) -> Self
    {
        Self { state }
    }
}

#[tonic::async_trait]
impl pb::exercise_service_server::ExerciseService for ExerciseServiceImpl
{
    /// 动作列表（可按部位筛选）
    ///
    /// 【教学：optional 参数怎么往下传】
    /// proto 的 `optional string body_part` → pb: Option<String>；
    /// REST 的查询函数要 `Option<&str>`，所以用 `.as_deref()`：
    ///   Option<String> → Option<&str>（借用，不复制字符串）
    /// 这个小适配器在本项目里到处都是（凡是"把 Option<String> 喂给收 &str 的函数"）。
    async fn list_exercises(
        &self,
        request: Request<pb::ListExercisesRequest>,
    ) -> Result<Response<pb::ListExercisesResponse>, Status>
    {
        let user = auth::require_user(&request, &self.state).await?;
        let req = request.into_inner();
        let pool = pool_of(&self.state).await;

        let list = rest_exercises::exercise_list(&pool, user.id, req.body_part.as_deref()).await?;

        Ok(Response::new(pb::ListExercisesResponse {
            exercises: list.iter().map(pb::Exercise::from).collect(),
        }))
    }

    /// 动作详情（含最近训练日期 + 历史最佳 1RM）
    async fn get_exercise(
        &self,
        request: Request<pb::GetExerciseRequest>,
    ) -> Result<Response<pb::Exercise>, Status>
    {
        let user = auth::require_user(&request, &self.state).await?;
        let req = request.into_inner();
        let pool = pool_of(&self.state).await;

        let out = rest_exercises::exercise_detail(&pool, user.id, req.id).await?;

        Ok(Response::new(pb::Exercise::from(&out)))
    }

    /// 创建动作
    ///
    /// 【教学：没传的字段用"服务器默认值"】
    /// proto 里 default_mode / bar_weight 等是 optional，转换时
    /// 复用 REST 层的默认值函数（exercises::default_mode 等），
    /// 所以两个出口的默认值永远一致（不会"REST 建出来是 20kg，gRPC 建出来是 0kg"）。
    async fn create_exercise(
        &self,
        request: Request<pb::CreateExerciseRequest>,
    ) -> Result<Response<pb::Exercise>, Status>
    {
        let user = auth::require_user(&request, &self.state).await?;
        let req = request.into_inner();
        let pool = pool_of(&self.state).await;

        let create_req = rest_exercises::ExerciseCreateReq::from(&req);
        let out = rest_exercises::exercise_create(&pool, user.id, &create_req).await?;

        Ok(Response::new(pb::Exercise::from(&out)))
    }

    /// 更新动作（PATCH 语义）
    async fn update_exercise(
        &self,
        request: Request<pb::UpdateExerciseRequest>,
    ) -> Result<Response<pb::Exercise>, Status>
    {
        let user = auth::require_user(&request, &self.state).await?;
        let req = request.into_inner();
        let pool = pool_of(&self.state).await;

        let update_req = rest_exercises::ExerciseUpdateReq::from(&req);
        let out = rest_exercises::exercise_update(&pool, user.id, req.id, &update_req).await?;

        Ok(Response::new(pb::Exercise::from(&out)))
    }

    /// 删除动作
    async fn delete_exercise(
        &self,
        request: Request<pb::DeleteExerciseRequest>,
    ) -> Result<Response<pb::Ack>, Status>
    {
        let user = auth::require_user(&request, &self.state).await?;
        let req = request.into_inner();
        let pool = pool_of(&self.state).await;

        rest_exercises::exercise_delete(&pool, user.id, req.id).await?;

        Ok(Response::new(pb::Ack { ok: true }))
    }

    // ============================================================
    // 服务器流式响应类型（tonic 的关联类型）
    // ============================================================
    /// 【教学：为什么流式方法要多写一个 `type`？】
    /// proto 里 `returns (stream Point)` 的意思不是"返回一个 Vec"，
    /// 而是"返回一个**异步序列**"：服务器可以一条一条发，
    /// 客户端可以一条一条收（收完一条就能画一条图）。
    /// Rust 里表达"异步序列"的类型是 `Stream`（≈ async 版的 Iterator）。
    /// tonic 生成的 trait 里有：
    ///     type StreamExerciseSeriesStream: Stream<Item = Result<Point, Status>> + Send;
    /// 我们必须为它指定一个**具体类型**（不能用 dyn：项目纪律 + 性能）。
    ///
    /// 【本项目的选择：tokio_stream::Iter】
    /// 先把数据算完放进 Vec，再用 `tokio_stream::iter()` 转成流。
    ///   ✅ 最简单、无 channel、无并发，符合"先跑通再优化"
    ///   ⚠️ 缺点：服务器端并没有"边查边发"（结果集很大时内存压力仍在服务器侧）
    /// 真正的流式（分页游标 + channel，或用 sqlx 的 fetch Stream）见 todo.md
    /// 的"待办：真流式导出"。**注意：即使服务器是"先算完再发"，
    /// 客户端拿到的仍然是逐条消息的流**（协议层面已经流式），
    /// 所以客户端的写法与真流式完全一致——将来换实现不影响契约。
    type StreamExerciseSeriesStream =
        tokio_stream::Iter<std::vec::IntoIter<Result<pb::ExerciseSeriesPoint, Status>>>;

    // ============================================================
    // StreamExerciseSeries（服务器流式，★ 挖空练习）
    // ============================================================
    /// 某动作的训练序列（1RM 折线图数据源），支持区间过滤
    ///
    /// 【实现步骤】
    /// 1. let user = auth::require_user(&request, &self.state).await?;
    /// 2. let req = request.into_inner();
    /// 3. let pool = pool_of(&self.state).await;
    /// 4. 复用 REST 的统计查询（已算好 1RM，按日期升序）：
    ///      rest_stats::exercise_stats_view(&pool, user.id, req.exercise_id).await?
    /// 5. 过滤 + 转换（**函数式链式写法**，别再写 for 循环）：
    ///      let points = stats.records.iter()
    ///          .filter(|r| req.from_date.as_deref().is_none_or(|from| r.date.as_str() >= from))
    ///          .filter(|r| req.to_date.as_deref().is_none_or(|to| r.date.as_str() <= to))
    ///          .map(|r| convert::series_point(req.exercise_id, r))
    ///          .collect::<Vec<_>>();
    ///    （'YYYY-MM-DD' 是"字典序 = 时间序"的格式，所以直接字符串比较即可，
    ///      不需要真的解析成日期——这是选日期格式时的实用理由。）
    /// 6. 把 Vec 变成流（.map(Ok) 给每个元素套上 Result 外壳）：
    ///      Ok(Response::new(tokio_stream::iter(
    ///          points.into_iter().map(Ok).collect::<Vec<_>>(),
    ///      )))
    ///    ⚠️ 注意流元素的类型必须是 `Result<Point, Status>`——
    ///       即使这里不可能出错，也要 `Ok(...)`（协议规定"每条消息都可能带状态"）。
    async fn stream_exercise_series(
        &self,
        request: Request<pb::ExerciseSeriesRequest>,
    ) -> Result<Response<Self::StreamExerciseSeriesStream>, Status>
    {
        let user = auth::require_user(&request, &self.state).await?;
        let req = request.into_inner();
        let pool = pool_of(&self.state).await;

        // 复用 REST 的统计查询（已算好 1RM，按日期升序）
        let stats = rest_stats::exercise_stats_view(&pool, user.id, req.exercise_id).await?;

        // 区间过滤 + 转换（链式适配器：filter → filter → map → collect）
        // 【教学：日期是 'YYYY-MM-DD'，字典序 = 时间序，直接比字符串即可】
        let points = stats
            .records
            .iter()
            .filter(|r| {
                req.from_date
                    .as_deref()
                    .is_none_or(|from| r.date.as_str() >= from)
            })
            .filter(|r| {
                req.to_date
                    .as_deref()
                    .is_none_or(|to| r.date.as_str() <= to)
            })
            .map(|r| convert::series_point(req.exercise_id, r))
            .collect::<Vec<_>>();

        // 变流：每个元素套上 Result 外壳（协议要求流元素是 Result<T, Status>）
        Ok(Response::new(tokio_stream::iter(
            points.into_iter().map(Ok).collect::<Vec<_>>(),
        )))
    }
}
