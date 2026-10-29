#[derive(Clone, Copy)]
pub struct Text
{
    pub title: &'static str,
    pub new_plan: &'static str,
    pub empty: &'static str,
    pub delete_confirm: &'static str,
    pub items: &'static str,
    pub create_title: &'static str,
    pub edit_title: &'static str,
    pub add_exercise: &'static str,
    pub no_items: &'static str,
    pub invalid_date: &'static str,
    pub invalid_number: &'static str,
    pub duplicate: &'static str,
    pub date_exists_hint: &'static str,
    pub plan_note: &'static str,
    pub item_note: &'static str,
    pub from_template: &'static str,
    pub no_templates: &'static str,
}

pub fn text(lang: crate::i18n::Lang) -> Text
{
    match lang
    {
        crate::i18n::Lang::Zh => Text {
            title: "训练计划",
            new_plan: "新建计划",
            empty: "本阶段还没有计划",
            delete_confirm: "确定删除这个计划？历史记录会保留",
            items: "个动作",
            create_title: "新建计划",
            edit_title: "编辑计划",
            add_exercise: "添加动作",
            no_items: "还没有动作，先添加",
            invalid_date: "日期格式应为 YYYY-MM-DD",
            invalid_number: "组数、次数、重量、休息必须是非负数字",
            duplicate: "这个动作已经在计划里了",
            date_exists_hint: "这个日期已经有计划了，请直接编辑那天的计划",
            plan_note: "当日备注",
            item_note: "动作备注",
            from_template: "从模板新建",
            no_templates: "本阶段还没有模板",
        },
        crate::i18n::Lang::En => Text {
            title: "Plans",
            new_plan: "New plan",
            empty: "No plans in this phase",
            delete_confirm: "Delete this plan? Training history stays",
            items: "exercises",
            create_title: "New plan",
            edit_title: "Edit plan",
            add_exercise: "Add exercise",
            no_items: "No exercise yet; add one",
            invalid_date: "Date must be YYYY-MM-DD",
            invalid_number: "Sets, reps, weight and rest must be non-negative numbers",
            duplicate: "This exercise is already in the plan",
            date_exists_hint: "A plan already exists on this date; edit it instead",
            plan_note: "Plan note",
            item_note: "Exercise note",
            from_template: "Plan from template",
            no_templates: "No templates in this phase",
        },
    }
}
