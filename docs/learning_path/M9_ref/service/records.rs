// ============================================================
// api/grpc/service/records.rs —— RecordService 实现（M9 第 6 步⑤）
// ============================================================
// 【教学：本文件是 M9 分量最重的一个，因为它演示了 gRPC 的**三种 RPC 类型**】
//   GetToday / UpsertRecord / GetDayRecords / UpdateRecord / DeleteRecord
//                                                   → 一元（unary）：一问一答
//   SubmitWorkout                                   → 客户端流（stream in）
//   LiveSession                                     → 双向流（stream in + out）
// 读完这 7 个方法，gRPC 的四种 RPC 类型你就都见过了（服务器流在 exercises/stats）。
//
// 【教学：客户端流 vs 循环调用一元方法，到底差在哪？】
//   循环调用：N 次往返 + N 次身份校验 + N 次连接复用开销，且没有"整体成功"语义
//   客户端流：1 次往返（N 条消息）+ 1 次身份校验 + 服务器能在收完后统一汇总
// 代价是客户端实现稍复杂（要先建流、再发完、最后等回包），
// 所以：**偶尔写一条 → 一元；批量提交一批 → 客户端流**。
//
// 【教学：注意三个"身份校验"的位置】
//   ① require_user 在流开始前调一次（不是每条消息都验）
//      —— 流建立时 metadata 已经送达，中途无法换身份（这是 HTTP/2 的语义）
//   ② 每条消息里的 plan_id/plan_item_id 仍会被 REST 层的 upsert 校验归属
//   ③ 客户端提前断开 → tx.send 返回 Err → 我们的任务结束（不会泄漏）
// ============================================================

use tokio_stream::wrappers::ReceiverStream;
use tonic::{Request, Response, Status, Streaming};

use super::pool_of;
use crate::{
    AppState,
    api::{
        grpc::{auth, convert, pb},
        rest::{records as rest_records, stats as rest_stats},
    },
    calc::epley_1rm,
};

#[derive(Clone)]
pub struct RecordServiceImpl
{
    pub(crate) state: AppState,
}

impl RecordServiceImpl
{
    pub fn new(state: AppState) -> Self
    {
        Self { state }
    }
}

#[tonic::async_trait]
impl pb::record_service_server::RecordService for RecordServiceImpl
{
    // ============================================================
    // GetToday（一元 RPC，★ 挖空练习）
    // ============================================================
    /// 今日卡片：进行中阶段 + 今日计划 + 每个动作的最近记录
    ///
    /// 【教学：这是 M10 客户端启动后的第一个调用】
    /// iced 打开后：登录 → GetToday → 画出今天的训练卡片。
    /// REST 的 /api/v1/today 和这里共用同一个 `today_view`（同一份 SQL、
    /// 同一套"没阶段就返回 None"的语义），所以两个客户端看到的数据永远一致。
    ///
    /// 【实现步骤】
    /// 1. let user = auth::require_user(&request, &self.state).await?;
    ///    （不需要 into_inner：这个请求没有字段）
    /// 2. let pool = pool_of(&self.state).await;
    /// 3. let view = rest_records::today_view(&pool, user.id).await?;
    /// 4. Ok(Response::new(pb::TodayView::from(&view)))
    ///    （转换的实现在 convert.rs，那里还有一处对应的挖空：
    ///      三层嵌套 + Option 的处理）
    async fn get_today(
        &self,
        request: Request<pb::GetTodayRequest>,
    ) -> Result<Response<pb::TodayView>, Status>
    {
        let user = auth::require_user(&request, &self.state).await?;
        let pool = pool_of(&self.state).await;

        let view = rest_records::today_view(&pool, user.id).await?;

        Ok(Response::new(pb::TodayView::from(&view)))
    }

    // ============================================================
    // UpsertRecord（一元 RPC，★ 挖空练习）
    // ============================================================
    /// 记录 upsert：同一计划项当天已有记录 → 更新；否则新建
    ///
    /// 【教学：“upsert”语义为什么重要？】
    /// 训练场景里用户可能：先记了 60kg*5 → 发现记错了改成 62.5kg*5。
    /// 如果每次都是 INSERT，数据库里会留下两条"同一天同一动作"的记录，
    /// 统计/日历/1RM 全都算重。
    /// 所以 REST 层（M8）用"先查最近记录，有则 UPDATE、无则 INSERT"实现 upsert，
    /// gRPC 直接复用——**同一个业务规则只实现一遍**。
    ///
    /// 【实现步骤】
    /// 1. let user = auth::require_user(&request, &self.state).await?;
    /// 2. let req = request.into_inner();
    /// 3. let pool = pool_of(&self.state).await;
    /// 4. 转成 REST 的输入类型：
    ///      let create_req = rest_records::RecordCreateReq::from(&req);
    /// 5. 调 REST 的 upsert（它内部会校验 计划归属 → 阶段未归档 →
    ///    计划项属于该计划 → 负数校验，再走 UPDATE/INSERT）：
    ///      let out = rest_records::record_upsert(
    ///          &pool, user.id, req.plan_id, req.plan_item_id, &create_req).await?;
    /// 6. Ok(Response::new(pb::Record::from(&out)))
    ///
    /// 【提示：注意 protobuf 的“编号与字段名”】
    ///   请求里的 plan_id / plan_item_id 在 proto 里编号是 1 和 2，
    ///   生成 Rust 后就是普通字段名 `req.plan_id`——编号只在线上传输时存在。
    async fn upsert_record(
        &self,
        request: Request<pb::UpsertRecordRequest>,
    ) -> Result<Response<pb::Record>, Status>
    {
        let user = auth::require_user(&request, &self.state).await?;
        let req = request.into_inner();
        let pool = pool_of(&self.state).await;

        let create_req = rest_records::RecordCreateReq::from(&req);
        let out =
            rest_records::record_upsert(&pool, user.id, req.plan_id, req.plan_item_id, &create_req)
                .await?;

        Ok(Response::new(pb::Record::from(&out)))
    }

    /// 某天全部记录（历史页；含部位与 1RM）
    ///
    /// 【教学：为什么用 records_range 而不是 stats::day_records？】
    /// 两者都能查"某天记录"，但：
    ///   · records_range 返回 RecordRow（记录 + 动作名 + 部位 + completed）
    ///     → 刚好能填满 pb::Record 的 15 个字段（转换零损失）
    ///   · day_records 返回 DayRecordOut（有部位，但没有 record_date/completed）
    /// gRPC 的契约是"客户端友好形状"，所以选字段更全的那个来源。
    /// （REST 的 /history/{date} 仍用 day_records，网页不多传字段，各取所需。）
    async fn get_day_records(
        &self,
        request: Request<pb::GetDayRecordsRequest>,
    ) -> Result<Response<pb::GetDayRecordsResponse>, Status>
    {
        let user = auth::require_user(&request, &self.state).await?;
        let req = request.into_inner();
        let pool = pool_of(&self.state).await;

        // from/to 都传同一天 = 精确匹配某天（区间上下界相等的退化用法）
        let rows =
            rest_records::records_range(&pool, user.id, Some(&req.date), Some(&req.date), None)
                .await?;

        Ok(Response::new(pb::GetDayRecordsResponse {
            // RecordRow → pb::Record（转换函数：字段最全，含 body_part）
            records: rows.iter().map(convert::record_from_row).collect(),
        }))
    }

    /// 更新单条记录（PATCH 语义：只改传了的字段）
    async fn update_record(
        &self,
        request: Request<pb::UpdateRecordRequest>,
    ) -> Result<Response<pb::Record>, Status>
    {
        let user = auth::require_user(&request, &self.state).await?;
        let req = request.into_inner();
        let pool = pool_of(&self.state).await;

        let update_req = rest_records::RecordUpdateReq::from(&req);
        let out = rest_records::record_update(&pool, user.id, req.id, &update_req).await?;

        Ok(Response::new(pb::Record::from(&out)))
    }

    /// 删除单条记录
    async fn delete_record(
        &self,
        request: Request<pb::DeleteRecordRequest>,
    ) -> Result<Response<pb::Ack>, Status>
    {
        let user = auth::require_user(&request, &self.state).await?;
        let req = request.into_inner();
        let pool = pool_of(&self.state).await;

        rest_records::record_delete(&pool, user.id, req.id).await?;

        Ok(Response::new(pb::Ack { ok: true }))
    }

    // ============================================================
    // SubmitWorkout（客户端流式，★ 挖空练习）
    // ============================================================
    /// 训练结束后批量提交一组记录，返回汇总（条数/总容量/最佳 1RM）
    ///
    /// 【教学：怎么读一个"客户端流"？】
    /// 入参类型是 `Request<Streaming<T>>`——注意不是 T，而是一个**读取器**：
    ///     let mut stream = request.into_inner();          // Streaming<UpsertRecordRequest>
    ///     while let Some(item) = stream.message().await?  // 每次拿一条；None = 客户端发完了
    ///     { ... }
    /// `message()` 的返回类型是 `Result<Option<T>, Status>`（双层包装）：
    ///   · Err(status)：流坏了（客户端崩溃/网络断）
    ///   · Ok(None)   ：正常结束（客户端 half-close）
    ///   · Ok(Some(t))：收到一条
    /// 所以 `while let Some(x) = stream.message().await?` 是最简写法：
    /// `?` 处理错误层，`Some/None` 处理结束层。
    ///
    /// 【教学：为什么这里逐条调 record_upsert，而不开一个事务？】
    /// SQLite 的连接池 + 事务嵌套有坑（REST 层的 impl 内部自己会 begin/commit）。
    /// 逐条 upsert 的语义是"部分成功也算数"，对训练记录是可接受的
    /// （用户重发一次就能补齐）。真要"全成功或全回滚"，
    /// 应该在 REST 层加一个 `records_bulk_upsert(&pool, user_id, &[RecordCreateReq])`
    /// 里做单事务——那是 M10/M11 的事，已记在 todo.md。
    ///
    /// 【实现步骤】
    /// 1. let user = auth::require_user(&request, &self.state).await?;
    ///    （守卫在"建流"时做一次，见文件头注释）
    /// 2. let mut stream = request.into_inner();
    /// 3. let pool = pool_of(&self.state).await;
    /// 4. 准备累加器：
    ///      let mut records: Vec<pb::Record> = Vec::new();
    ///      let mut total_volume = 0.0f64;
    ///      let mut best_1rm = 0.0f64;
    /// 5. 循环读完整个流：
    ///      while let Some(item) = stream.message().await?
    ///      {
    ///          let create_req = rest_records::RecordCreateReq::from(&item);
    ///          let out = rest_records::record_upsert(
    ///              &pool, user.id, item.plan_id, item.plan_item_id, &create_req).await?;
    ///          // 【教学：容量（volume）= 重量 × 组数 × 次数，健身界通用指标】
    ///          total_volume += out.weight * out.sets as f64 * out.reps as f64;
    ///          best_1rm = best_1rm.max(epley_1rm(out.weight, out.reps));
    ///          records.push(pb::Record::from(&out));
    ///      }
    /// 6. 组装汇总返回（⚠️ 注意 best_1rm 是 proto 的 optional：
    ///        "没有任何有效记录"应该是**不传**，而不是传 0.0：
    ///        let best = (best_1rm > 0.0).then_some(best_1rm);
    ///        Ok(Response::new(pb::SubmitWorkoutSummary {
    ///            saved_count: records.len() as i32,
    ///            total_volume,
    ///            best_1rm: best,
    ///            records,
    ///        })))
    async fn submit_workout(
        &self,
        request: Request<Streaming<pb::UpsertRecordRequest>>,
    ) -> Result<Response<pb::SubmitWorkoutSummary>, Status>
    {
        let user = auth::require_user(&request, &self.state).await?;
        let mut stream = request.into_inner();
        let pool = pool_of(&self.state).await;

        let mut records: Vec<pb::Record> = Vec::new();
        let mut total_volume = 0.0f64;
        let mut best_1rm = 0.0f64;

        // 读完整个流：`?` 处理错误层，Some/None 处理结束层
        while let Some(item) = stream.message().await?
        {
            let create_req = rest_records::RecordCreateReq::from(&item);
            let out = rest_records::record_upsert(
                &pool,
                user.id,
                item.plan_id,
                item.plan_item_id,
                &create_req,
            )
            .await?;

            // 容量 = 重量 × 组数 × 次数（健身界通用指标）
            total_volume += out.weight * out.sets as f64 * out.reps as f64;
            best_1rm = best_1rm.max(epley_1rm(out.weight, out.reps));
            records.push(pb::Record::from(&out));
        }

        Ok(Response::new(pb::SubmitWorkoutSummary {
            saved_count: records.len() as i32,
            total_volume,
            // 没有任何有效记录 → 不传（而不是传 0.0）
            best_1rm: (best_1rm > 0.0).then_some(best_1rm),
            records,
        }))
    }

    // ============================================================
    // 双向流的响应类型（同服务器流：要给关联类型指定具体类型）
    // ============================================================
    /// 用 mpsc 通道把"处理器任务"和"响应流"接起来（详见下方 live_session）
    type LiveSessionStream = ReceiverStream<Result<pb::LiveSetFeedback, Status>>;

    // ============================================================
    // LiveSession（双向流，已实现，读作参考）
    // ============================================================
    /// 训练中"一组一上报、一组一回执"（实时反馈 1RM / 破纪录提示）
    ///
    /// 【教学：双向流为什么不能像服务器流那样"先算完再 iter"？】
    /// 因为回执依赖请求，而请求是**在时间上陆续到达**的：
    /// 客户端发第 1 组时，服务器还不知道第 2 组是什么。
    /// 所以这里必须"边收边发"——Rust 里的标准做法：
    ///     mpsc::channel 建一个管道 → 一个任务写、响应流读
    ///   ① `tokio::spawn` 一个任务：循环 inbound.message() 落库，把回执 send 进 channel
    ///   ② 方法立刻返回 `ReceiverStream::new(rx)`：tonic 从管道里取消息发回客户端
    /// 两个任务并发跑，谁快等谁（channel 的容量 16 就是"最多积压 16 条"，
    /// 客户端读得慢时服务器会自动慢下来 → 天然的背压/backpressure）。
    ///
    /// 【实现步骤（本方法已写好，请对照读）】
    /// 1. 守卫（流建立时校验一次）
    /// 2. 先 into_inner 拿 inbound；把 state clone 一份给 spawn 的任务
    ///    （AppState 内部是 Arc，clone 极廉价）
    /// 3. 建 channel：tokio::sync::mpsc::channel(16)
    /// 4. spawn 任务：循环读消息 → upsert → 查历史最佳 → 发回执
    /// 5. 返回 ReceiverStream::new(rx)
    ///
    /// 【教学：客户端断开怎么处理？】
    /// tx.send(...) 返回 Err 表示"接收端没了"（客户端走了）→ break 结束任务。
    /// 忘记处理这一条，服务器会留下一个永远等待的任务（任务泄漏）。
    async fn live_session(
        &self,
        request: Request<Streaming<pb::UpsertRecordRequest>>,
    ) -> Result<Response<Self::LiveSessionStream>, Status>
    {
        // ① 身份校验：流建立时一次（之后每条消息都视为同一用户）
        let user = auth::require_user(&request, &self.state).await?;
        let user_id = user.id;

        // ② 拿出"请求读取器"，状态 clone 一份交给后台任务
        let mut inbound = request.into_inner();
        let state = self.state.clone();

        // ③ 建管道：容量 16 表示最多缓冲 16 条回执（背压）
        let (tx, rx) = tokio::sync::mpsc::channel(16);

        // ④ 后台任务：边收边查边发
        tokio::spawn(async move {
            let pool = pool_of(&state).await;
            let mut completed_count = 0i32;

            loop
            {
                match inbound.message().await
                {
                    // 客户端发完了（half-close）→ 结束任务
                    Ok(None) => break,
                    // 流出错（客户端崩溃/网络断）→ 记一条日志结束
                    Err(status) =>
                    {
                        tracing::warn!("LiveSession 读取失败: {status}");
                        break;
                    },
                    Ok(Some(item)) =>
                    {
                        let create_req = rest_records::RecordCreateReq::from(&item);
                        let result = rest_records::record_upsert(
                            &pool,
                            user_id,
                            item.plan_id,
                            item.plan_item_id,
                            &create_req,
                        )
                        .await;

                        // 落库失败：把状态码发回客户端，然后结束这条流
                        // （错误无法"跳过继续"：客户端需要知道这一组没存上）
                        let out = match result
                        {
                            Ok(out) => out,
                            Err(e) =>
                            {
                                let _ = tx.send(Err(Status::from(e))).await;
                                break;
                            },
                        };

                        // 本组 1RM（实时算）
                        let one_rm = epley_1rm(out.weight, out.reps);

                        // 该动作历史最佳（含刚存的这组；查询失败就退回本组值）
                        // ⚠️ 教学实现：每组重查一次全量记录（个人数据量下没问题）；
                        //    量大了应改成增量维护 PR 或加缓存（todo.md 已记录）。
                        let best_1rm =
                            match rest_stats::exercise_stats_view(&pool, user_id, out.exercise_id)
                                .await
                            {
                                Ok(stats) => stats.best_1rm,
                                Err(_) => one_rm,
                            };

                        if out.completed
                        {
                            completed_count += 1;
                        }

                        let feedback = pb::LiveSetFeedback {
                            // 【教学：消息字段是 Option】proto 里 `Record saved = 1;`
                            // 生成 Rust 后是 `Option<pb::Record>`（消息类型默认"可不传"），
                            // 所以要 Some(...) 包一层。
                            saved: Some(pb::Record::from(&out)),
                            best_1rm,
                            // 本组就是历史最佳（"破纪录"提示）
                            new_pr: one_rm > 0.0 && one_rm >= best_1rm,
                            completed_count,
                        };

                        // ⑤ 发回执；客户端断开 → send 失败 → 结束任务
                        if tx.send(Ok(feedback)).await.is_err()
                        {
                            break;
                        }
                    },
                }
            }
        });

        // 响应流：tonic 从这里取消息发给客户端
        Ok(Response::new(ReceiverStream::new(rx)))
    }
}
