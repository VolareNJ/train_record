#[derive(Clone, Copy)]
pub struct Text
{
    pub phase_missing: &'static str,
    pub phase_missing_hint: &'static str,
    pub plan_missing: &'static str,
    pub plan_missing_hint: &'static str,
    pub create_plan: &'static str,
    pub manage_phase: &'static str,
    pub trained: &'static str,
    pub untrained: &'static str,
    pub last_record: &'static str,
    pub no_last: &'static str,
    pub strategy_hint: &'static str,
    pub plan_note: &'static str,
    pub tap_to_log: &'static str,
    pub today_plan: &'static str,
    pub manage_plan: &'static str,
}

pub fn text(lang: crate::i18n::Lang) -> Text
{
    match lang
    {
        crate::i18n::Lang::Zh => Text {
            phase_missing: "还没有进行中的阶段",
            phase_missing_hint: "先建一个阶段，计划与记录都挂在阶段下",
            plan_missing: "今天还没有计划",
            plan_missing_hint: "到阶段里挑模板或手动排今天的动作",
            create_plan: "去排今天的计划",
            manage_phase: "阶段管理",
            trained: "已训练",
            untrained: "未训练",
            last_record: "上次",
            no_last: "还没练过这个动作",
            strategy_hint: "上次策略",
            plan_note: "今日备注",
            tap_to_log: "点动作卡片记录",
            today_plan: "今日计划",
            manage_plan: "管理计划",
        },
        crate::i18n::Lang::En => Text {
            phase_missing: "No active phase",
            phase_missing_hint: "Create a phase first; plans and records live under it",
            plan_missing: "No plan for today",
            plan_missing_hint: "Pick a template or arrange today's exercises",
            create_plan: "Arrange today",
            manage_phase: "Phases",
            trained: "Trained",
            untrained: "Not trained",
            last_record: "Last",
            no_last: "No record for this exercise yet",
            strategy_hint: "Last strategy",
            plan_note: "Today's note",
            tap_to_log: "Tap a card to log",
            today_plan: "Today's plan",
            manage_plan: "Manage plan",
        },
    }
}
