// ============================================================
// api/grpc/service/plans.rs —— PlanService 实现（M9 第 6 步④）
// ============================================================
// 【教学：本文件的亮点是"嵌套 repeated 写入"】
// 创建计划时客户端一次传进来一串动作项：
//   CreatePlanRequest { phase_id, date, note, items: [PlanItemInput, ...] }
// 服务端的任务是：把这串 proto 消息转成 REST 的输入类型，再交给 REST 层的事务
// （REST 层负责 begin → 逐条 INSERT → commit，见 rest/plans.rs 的 plan_create_impl）。
//
// 【教学：为什么事务不在这里开？】
// "先写 plans 表、再写 plan_items 表"是一个**业务不变量**
// （不能出现"有计划但没动作项"的中间状态）。
// 这种不变量属于业务层（rest 层），不属于协议层（gRPC）：
// 如果哪天再加一个 CLI 出口，它会自动继承同一套事务保证。
// 协议层只负责把请求翻译成"业务层认识的样子"。
//
// 【教学：数据隔离怎么保证的？（回忆 AGENTS.md 的纪律）】
// 每个写操作都要先校验"这个阶段/模板/计划属于当前用户"：
//   phase_id  → phase_create_impl 内部 verify_phase
//   plan_id   → plan_update_impl 内部 verify_plan（JOIN phases 查 user_id）
// 所以 gRPC 层不需要自己写任何 WHERE user_id = ?，
// 但也意味着：**绝不能绕过 REST 层的这些函数自己写 SQL**（一条错误路径就够了）。
// ============================================================

use tonic::{Request, Response, Status};

use super::pool_of;
use crate::{
    AppState,
    api::{
        grpc::{auth, pb},
        rest::plans as rest_plans,
    },
};

#[derive(Clone)]
pub struct PlanServiceImpl
{
    pub(crate) state: AppState,
}

impl PlanServiceImpl
{
    pub fn new(state: AppState) -> Self
    {
        Self { state }
    }
}

// ============================================================
// 【教学：为什么 REST 层的函数名带 `_impl` 后缀？】
// rest/plans.rs 里的 handler 已经叫 `plan_create` / `template_list`……，
// 而 Rust **没有函数重载**（同名函数不能共存），所以抽出来的共享函数
// 只能换个名字：`plan_create_impl`。
// 这是"给老代码加共享层"时的现实妥协，两种命名方案：
//   a. 共享函数加后缀（本项目的选择）：handler 名字保持不动 → 老代码零改动
//   b. handler 改名（如 plan_create_handler）：语义更清楚，但要改路由表
// 选 a 的理由：M8 的代码和文档都引用旧名字，改名会让 diff 噪声变大。
// （对比 phases.rs / exercises.rs：那边的 handler 叫 list/create/detail，
//   不冲突，所以共享函数就是 phase_list / exercise_list，没有后缀。）
// ============================================================

#[tonic::async_trait]
impl pb::plan_service_server::PlanService for PlanServiceImpl
{
    // ------------------------------------------------------------
    // 模板
    // ------------------------------------------------------------
    /// 阶段下的模板列表（每个模板含有序动作项）
    async fn list_templates(
        &self,
        request: Request<pb::ListTemplatesRequest>,
    ) -> Result<Response<pb::ListTemplatesResponse>, Status>
    {
        let user = auth::require_user(&request, &self.state).await?;
        let req = request.into_inner();
        let pool = pool_of(&self.state).await;

        let list = rest_plans::template_list_impl(&pool, user.id, req.phase_id).await?;

        Ok(Response::new(pb::ListTemplatesResponse {
            templates: list.iter().map(pb::Template::from).collect(),
        }))
    }

    /// 创建模板（含动作项，顺序 = items 数组顺序）
    async fn create_template(
        &self,
        request: Request<pb::CreateTemplateRequest>,
    ) -> Result<Response<pb::Template>, Status>
    {
        let user = auth::require_user(&request, &self.state).await?;
        let req = request.into_inner();
        let pool = pool_of(&self.state).await;

        let template_req = rest_plans::TemplateReq::from(&req);
        let out =
            rest_plans::template_create_impl(&pool, user.id, req.phase_id, &template_req).await?;

        Ok(Response::new(pb::Template::from(&out)))
    }

    /// 更新模板（全量替换 name + items）
    async fn update_template(
        &self,
        request: Request<pb::UpdateTemplateRequest>,
    ) -> Result<Response<pb::Template>, Status>
    {
        let user = auth::require_user(&request, &self.state).await?;
        let req = request.into_inner();
        let pool = pool_of(&self.state).await;

        let template_req = rest_plans::TemplateReq::from(&req);
        let out = rest_plans::template_update_impl(&pool, user.id, req.id, &template_req).await?;

        Ok(Response::new(pb::Template::from(&out)))
    }

    /// 删除模板
    async fn delete_template(
        &self,
        request: Request<pb::DeleteTemplateRequest>,
    ) -> Result<Response<pb::Ack>, Status>
    {
        let user = auth::require_user(&request, &self.state).await?;
        let req = request.into_inner();
        let pool = pool_of(&self.state).await;

        rest_plans::template_delete_impl(&pool, user.id, req.id).await?;

        Ok(Response::new(pb::Ack { ok: true }))
    }

    // ------------------------------------------------------------
    // 计划
    // ------------------------------------------------------------
    /// 阶段下的计划列表（可选按日期过滤）
    async fn list_plans(
        &self,
        request: Request<pb::ListPlansRequest>,
    ) -> Result<Response<pb::ListPlansResponse>, Status>
    {
        let user = auth::require_user(&request, &self.state).await?;
        let req = request.into_inner();
        let pool = pool_of(&self.state).await;

        let list =
            rest_plans::plan_list_impl(&pool, user.id, req.phase_id, req.date.as_deref()).await?;

        Ok(Response::new(pb::ListPlansResponse {
            plans: list.iter().map(pb::Plan::from).collect(),
        }))
    }

    /// 计划详情（含动作项）
    async fn get_plan(
        &self,
        request: Request<pb::GetPlanRequest>,
    ) -> Result<Response<pb::Plan>, Status>
    {
        let user = auth::require_user(&request, &self.state).await?;
        let req = request.into_inner();
        let pool = pool_of(&self.state).await;

        let out = rest_plans::plan_detail_impl(&pool, user.id, req.id).await?;

        Ok(Response::new(pb::Plan::from(&out)))
    }

    // ============================================================
    // CreatePlan（一元 RPC，★ 挖空练习）
    // ============================================================
    /// 创建今日/某日计划（含动作项）
    ///
    /// 【教学：这里的"翻译"比 phase 复杂一点，因为 items 是嵌套 repeated】
    ///   pb:  CreatePlanRequest { phase_id, date, note, items: Vec<PlanItemInput> }
    ///   REST: PlanReq { date, note, items: Vec<PlanItemReq> }
    /// 注意 phase_id **不在** PlanReq 里——REST 的 handler 是从 URL 路径拿的
    /// （`/phases/{phase_id}/plans`），所以它是**单独的函数参数**。
    /// 这类"字段在协议里的位置不同"是跨协议复用的常态：
    /// 契约层（proto）把它放在 body 里，是因为 gRPC 没有"路径参数"这个概念。
    ///
    /// 【实现步骤】
    /// 1. let user = auth::require_user(&request, &self.state).await?;
    /// 2. let req = request.into_inner();
    /// 3. let pool = pool_of(&self.state).await;
    /// 4. 转输入类型（items 的嵌套转换由 convert.rs 里的 From 链自动完成）：
    ///      let plan_req = rest_plans::PlanReq::from(&req);
    /// 5. 交给 REST 层的事务实现（校验阶段归属/未归档 → 事务插入 → 提交）：
    ///      let out = rest_plans::plan_create_impl(&pool, user.id, req.phase_id, &plan_req).await?;
    /// 6. Ok(Response::new(pb::Plan::from(&out)))
    ///
    /// 【提示：可以先看 update_plan 的写法（已实现），它和这里几乎一样，
    ///   只是 id 与 phase_id 的位置不同。】
    #[allow(unused_variables)]
    async fn create_plan(
        &self,
        request: Request<pb::CreatePlanRequest>,
    ) -> Result<Response<pb::Plan>, Status>
    {
        // 【实现步骤】见上方注释
        todo!("M9 练习：CreatePlan 实现") // 【待实现】
    }

    /// 更新计划（全量替换 date + note + items）
    ///
    /// 【教学：为什么更新是"全量替换"而不是逐项 PATCH？】
    /// 计划项是"有序列表"，逐项 PATCH 会带来一堆语义问题
    /// （顺序怎么改？删一项怎么表达？插入放哪？）。
    /// REST 层（M4/M8）已经定了"先删后插 + 按数组顺序重建 sort_order"的做法，
    /// gRPC 沿用同一语义：**传什么就是全部**。
    /// 想局部改一项 → 先 GetPlan 拿到全部，改完再 UpdatePlan（客户端拉-改-推）。
    async fn update_plan(
        &self,
        request: Request<pb::UpdatePlanRequest>,
    ) -> Result<Response<pb::Plan>, Status>
    {
        let user = auth::require_user(&request, &self.state).await?;
        let req = request.into_inner();
        let pool = pool_of(&self.state).await;

        let plan_req = rest_plans::PlanReq::from(&req);
        let out = rest_plans::plan_update_impl(&pool, user.id, req.id, &plan_req).await?;

        Ok(Response::new(pb::Plan::from(&out)))
    }

    /// 删除计划
    async fn delete_plan(
        &self,
        request: Request<pb::DeletePlanRequest>,
    ) -> Result<Response<pb::Ack>, Status>
    {
        let user = auth::require_user(&request, &self.state).await?;
        let req = request.into_inner();
        let pool = pool_of(&self.state).await;

        rest_plans::plan_delete_impl(&pool, user.id, req.id).await?;

        Ok(Response::new(pb::Ack { ok: true }))
    }
}
