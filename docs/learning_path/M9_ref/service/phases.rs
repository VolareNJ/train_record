// ============================================================
// api/grpc/service/phases.rs —— PhaseService 实现（M9 第 6 步②）
// ============================================================
// 【教学：这个文件里 5 个方法，只有第 5 个挖空，前 4 个是"照着抄"的模板】
// 请按顺序读前 4 个方法的实现，观察它们如何共用同一个骨架构：
//   ① 守卫 → ② 调 REST 层的共享查询函数 → ③ 转消息
// 你会发现"加一个 RPC"在这种结构下几乎是机械劳动——
// 这正是 REST 层提前抽出 pub(crate) 查询函数的回报。
//
// 【教学：注意 `?` 在这里的两种含义（本项目最常用的两个转换）】
//   guard::require_user(...).await?     Status 直接向上抛
//   rest_phases::phase_list(...).await? ApiError **自动**变成 Status（靠 error.rs 的 From）
// 写代码时不用手动 .map_err —— 前提是 error.rs 里那个 From 实现写对了。
// ============================================================

#[derive(Clone)]
pub struct PhaseServiceImpl
{
    pub(crate) state: crate::AppState,
}

impl PhaseServiceImpl
{
    pub fn new(state: crate::AppState) -> Self
{
        Self { state }
}
}

#[tonic::async_trait]
impl crate::api::grpc::pb::phase_service_server::PhaseService for PhaseServiceImpl
{
    /// 阶段列表（含"已坚持 N 天"）
    ///
    /// 【教学：repeated 字段怎么填】
    /// pb 的 `repeated Phase phases` 生成的是 `Vec<pb::Phase>`，
    /// 而 REST 给的是 `Vec<PhaseOut>`。
    /// 一行迭代器解决：`.iter().map(pb::Phase::from).collect()`
    /// （因为 convert.rs 里有 `impl From<&PhaseOut> for pb::Phase`）
    async fn list_phases(
        &self,
        request: tonic::Request<crate::api::grpc::pb::ListPhasesRequest>,
    ) -> Result<tonic::Response<crate::api::grpc::pb::ListPhasesResponse>, tonic::Status>
{
        let user = crate::api::grpc::auth::require_user(&request, &self.state).await?;
        let pool = crate::api::grpc::service::pool_of(&self.state).await;

        let phases = crate::api::rest::phases::phase_list(&pool, user.id).await?;

        Ok(tonic::Response::new(
            crate::api::grpc::pb::ListPhasesResponse {
                phases: phases
                    .iter()
                    .map(crate::api::grpc::pb::Phase::from)
                    .collect(),
    },
        ))
}

    /// 阶段详情
    async fn get_phase(
        &self,
        request: tonic::Request<crate::api::grpc::pb::GetPhaseRequest>,
    ) -> Result<tonic::Response<crate::api::grpc::pb::Phase>, tonic::Status>
{
        let user = crate::api::grpc::auth::require_user(&request, &self.state).await?;
        let req = request.into_inner();
        let pool = crate::api::grpc::service::pool_of(&self.state).await;

        let out = crate::api::rest::phases::phase_detail(&pool, user.id, req.id).await?;

        Ok(tonic::Response::new(crate::api::grpc::pb::Phase::from(
            &out,
        )))
}

    /// 创建阶段
    ///
    /// 【教学：请求消息 → REST 请求体的转换】
    /// 下游复用 REST 的 `phase_create`，它收的是 `PhaseCreateReq`。
    /// convert.rs 里写了 `From<&pb::CreatePhaseRequest>`，所以：
    ///     rest_phases::PhaseCreateReq::from(&req)
    async fn create_phase(
        &self,
        request: tonic::Request<crate::api::grpc::pb::CreatePhaseRequest>,
    ) -> Result<tonic::Response<crate::api::grpc::pb::Phase>, tonic::Status>
{
        let user = crate::api::grpc::auth::require_user(&request, &self.state).await?;
        let req = request.into_inner();
        let pool = crate::api::grpc::service::pool_of(&self.state).await;

        let create_req = crate::api::rest::phases::PhaseCreateReq::from(&req);
        let out = crate::api::rest::phases::phase_create(&pool, user.id, &create_req).await?;

        Ok(tonic::Response::new(crate::api::grpc::pb::Phase::from(
            &out,
        )))
}

    /// 更新阶段（PATCH 语义：只改传了的字段）
    ///
    /// 【教学：optional 字段一路传递的过程】
    ///   proto: optional string name  → pb: Option<String>
    ///   → 转换 → REST: Option<String>（#[serde(default)]）
    ///   → SQL: 只 UPDATE 非 None 的列
    /// 整条链上 None 的语义始终是"没传/不改"，没有一步丢失信息——
    /// 这就是 proto3 `optional`（显式存在性）存在的意义。
    async fn update_phase(
        &self,
        request: tonic::Request<crate::api::grpc::pb::UpdatePhaseRequest>,
    ) -> Result<tonic::Response<crate::api::grpc::pb::Phase>, tonic::Status>
{
        let user = crate::api::grpc::auth::require_user(&request, &self.state).await?;
        let req = request.into_inner();
        let pool = crate::api::grpc::service::pool_of(&self.state).await;

        let update_req = crate::api::rest::phases::PhaseUpdateReq::from(&req);
        let out =
            crate::api::rest::phases::phase_update(&pool, user.id, req.id, &update_req).await?;

        Ok(tonic::Response::new(crate::api::grpc::pb::Phase::from(
            &out,
        )))
}

// ============================================================
    // SetPhaseArchived（一元 RPC， 挖空练习）
// ============================================================
    /// 归档 / 启用阶段（archived = true 归档，false 启用）
    ///
    /// 【教学：一个方法顶 REST 两个端点】
    /// REST 是 POST /phases/{id}/archive 与 /unarchive 两条路由；
    /// 这里合并为一个"幂等设置"：传什么就是什么。
    /// 好处：客户端少记一个方法；重复调用也不会出错（archive 两次结果一样）。
    ///
    /// 【实现步骤】
    /// 1. let user = auth::require_user(&request, &self.state).await?;
    /// 2. let req = request.into_inner();
    /// 3. let pool = pool_of(&self.state).await;
    /// 4. 调 REST 层抽好的函数（它同时做了"存在 + 归属 + 未归档"的校验）：
    ///      rest_phases::phase_set_archived(&pool, user.id, req.id, req.archived).await?
    /// 5. Ok(Response::new(pb::Phase::from(&out)))
    async fn set_phase_archived(
        &self,
        request: tonic::Request<crate::api::grpc::pb::SetPhaseArchivedRequest>,
    ) -> Result<tonic::Response<crate::api::grpc::pb::Phase>, tonic::Status>
{
        let user = crate::api::grpc::auth::require_user(&request, &self.state).await?;
        let req = request.into_inner();
        let pool = crate::api::grpc::service::pool_of(&self.state).await;

        let out =
            crate::api::rest::phases::phase_set_archived(&pool, user.id, req.id, req.archived)
                .await?;

        Ok(tonic::Response::new(crate::api::grpc::pb::Phase::from(
            &out,
        )))
}
}
