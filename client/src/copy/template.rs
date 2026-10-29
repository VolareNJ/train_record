#[derive(Clone, Copy)]
pub struct Text
{
    pub title: &'static str,
    pub new_template: &'static str,
    pub empty: &'static str,
    pub use_template: &'static str,
    pub delete_confirm: &'static str,
    pub items: &'static str,
    pub create_title: &'static str,
    pub edit_title: &'static str,
    pub pick_exercise: &'static str,
    pub selected: &'static str,
    pub move_up: &'static str,
    pub move_down: &'static str,
    pub no_items: &'static str,
    pub add_hint: &'static str,
}

pub fn text(lang: crate::i18n::Lang) -> Text
{
    match lang
    {
        crate::i18n::Lang::Zh => Text {
            title: "训练模板",
            new_template: "新建模板",
            empty: "本阶段还没有模板",
            use_template: "用此模板建计划",
            delete_confirm: "确定删除这个模板？",
            items: "个动作",
            create_title: "新建模板",
            edit_title: "编辑模板",
            pick_exercise: "选择动作",
            selected: "已选动作",
            move_up: "上移",
            move_down: "下移",
            no_items: "还没有选动作",
            add_hint: "按训练顺序添加动作，可上移下移调整",
        },
        crate::i18n::Lang::En => Text {
            title: "Templates",
            new_template: "New template",
            empty: "No templates in this phase",
            use_template: "Plan from template",
            delete_confirm: "Delete this template?",
            items: "exercises",
            create_title: "New template",
            edit_title: "Edit template",
            pick_exercise: "Pick exercise",
            selected: "Selected",
            move_up: "Up",
            move_down: "Down",
            no_items: "No exercise selected",
            add_hint: "Add exercises in training order; move them up or down",
        },
    }
}
