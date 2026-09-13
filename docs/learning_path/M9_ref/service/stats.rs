// ============================================================
// api/grpc/service/stats.rs —— StatsService 实现（M9 第 6 步⑥）
// ============================================================
// 【教学：统计类接口为什么特别适合流式？】
// 统计/导出天然是"一大串数据"：
//   · 日历：一个月最多 31 个日期（小，一元足够）
//   · 训练记录导出：一年可能几千条（大，适合流式）
// 判断标准不是"数据大不大"，而是**客户端能不能边收边用**：
//   · 画折线图 → 收到一个点就能画一个（流式 ✓，见 exercises::stream_exercise_series）
//   · 导出到文件 → 收到一条就能写一条（流式 ✓，就是本文件的 stream_records）
//   · 画日历 → 必须先知道整月（一元 ✓）
// ============================================================

use tonic::{Request, Response, Status};

use super::pool_of;
use crate::{
    AppState,
    api::{
        grpc::{auth, convert, pb},
        rest::{records as rest_records, stats as rest_stats},
    },
};

#[derive(Clone)]
pub struct StatsServiceImpl
{
    pub(crate) state: AppState,
}

impl StatsServiceImpl
{
    pub fn new(state: AppState) -> Self
    {
        Self { state }
    }
}

#[tonic::async_trait]
impl pb::stats_service_server::StatsService for StatsServiceImpl
{
    /// 历史日历：某月有训练的日期列表（不传年月 = 当前年月）
    ///
    /// 【教学：optional 参数的"缺省用服务器当前值"】
    /// proto 里 year/month 是 optional：客户端不传，服务器用数据库里的"今天"
    /// （SELECT strftime('%Y-%m','now','localtime')）——注意是**服务器时区**，
    /// 这也是 REST 层早已定下的行为（时区纪律见 structure.md §2.13）。
    /// 客户端要"看别的月份"就显式传 '2026' / '03'。
    async fn get_calendar(
        &self,
        request: Request<pb::GetCalendarRequest>,
    ) -> Result<Response<pb::CalendarView>, Status>
    {
        let user = auth::require_user(&request, &self.state).await?;
        let req = request.into_inner();
        let pool = pool_of(&self.state).await;

        let out =
            rest_stats::calendar_view(&pool, user.id, req.year.as_deref(), req.month.as_deref())
                .await?;

        Ok(Response::new(pb::CalendarView::from(&out)))
    }

    // ============================================================
    // 服务器流式响应类型（同 exercises.rs 的解释）
    // ============================================================
    type StreamRecordsStream = tokio_stream::Iter<std::vec::IntoIter<Result<pb::Record, Status>>>;

    // ============================================================
    // StreamRecords（服务器流式，★ 挖空练习）
    // ============================================================
    /// 按条件导出训练记录（流式：一条一条发）
    ///
    /// 【教学：这个 RPC 是"REST 没有的能力"的典型例子】
    /// REST 版导出必须：查全部 → 序列化成一个大 JSON → 一次写完。
    /// 记录多时：服务器内存峰值高、客户端要等整体完成才能开始处理。
    /// gRPC 流式版：客户端收到第一条就能写进文件/画进图，
    /// 服务器也不必（在真正的流式实现下）把全部结果留在内存。
    ///
    /// 【实现步骤】
    /// 1. let user = auth::require_user(&request, &self.state).await?;
    /// 2. let req = request.into_inner();
    /// 3. let pool = pool_of(&self.state).await;
    /// 4. 复用 REST 层的区间查询（它内部用 JOIN exercises 保证数据隔离，
    ///    并对 from/to 做了日期格式校验）：
    ///      let rows = rest_records::records_range(
    ///          &pool,
    ///          user.id,
    ///          req.from_date.as_deref(),
    ///          req.to_date.as_deref(),
    ///          req.exercise_id,
    ///      ).await?;
    /// 5. 转消息 + 变流：
    ///      let items = rows.iter()
    ///          .map(pb::Record::from)          // RecordRow → pb::Record
    ///          .map(Ok)                         // 每条包 Result（协议要求）
    ///          .collect::<Vec<_>>();
    ///      Ok(Response::new(tokio_stream::iter(items)))
    ///    （注意这里用了两级 map：先是类型转换，再是加 Result 外壳——
    ///      把"转换"和"协议包装"分开写，读起来比塞在一个闭包里清楚。）
    ///
    /// 【提示：过滤器都交给 SQL 了，为什么不是在 Rust 里 filter？】
    /// 因为过滤条件是**数据库擅长的**（走索引、不浪费传输），
    /// 能在 SQL 层过滤就不要先拉回内存再筛。Rust 侧的迭代器过滤
    /// 留给"SQL 拿不到的东西"（比如 exercise_stats 的日期区间裁剪——
    /// 那个查询是按动作全量取的，见 exercises.rs 的实现步骤）。
    async fn stream_records(
        &self,
        request: Request<pb::StreamRecordsRequest>,
    ) -> Result<Response<Self::StreamRecordsStream>, Status>
    {
        let user = auth::require_user(&request, &self.state).await?;
        let req = request.into_inner();
        let pool = pool_of(&self.state).await;

        // 过滤都交给 SQL（走索引、不白传数据）
        let rows = rest_records::records_range(
            &pool,
            user.id,
            req.from_date.as_deref(),
            req.to_date.as_deref(),
            req.exercise_id,
        )
        .await?;

        // 转换 + 协议包装（分开两级 map，读起来更清楚）
        let items = rows
            .iter()
            .map(convert::record_from_row)
            .map(Ok)
            .collect::<Vec<_>>();

        Ok(Response::new(tokio_stream::iter(items)))
    }
}
