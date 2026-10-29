type LoadedData = (
    Vec<crate::model::Plan>,
    Vec<crate::model::Template>,
    Vec<crate::model::Exercise>,
);

#[derive(Debug, Clone, Default)]
pub struct State
{
    pub phase_id: i64,
    pub phase_name: String,
    pub plans: Vec<crate::model::Plan>,
    pub templates: Vec<crate::model::Template>,
    pub exercises: Vec<crate::model::Exercise>,
    pub loading: bool,
    pub error: Option<String>,
    pub confirm_delete: Option<i64>,
}

#[derive(Debug, Clone)]
pub enum Message
{
    Refresh,
    Loaded(Result<LoadedData, crate::api::ApiError>),
    New,
    Edit(i64),
    FromTemplate(i64),
    Delete(i64),
    DeleteConfirmed(i64),
    Deleted(Result<serde_json::Value, crate::api::ApiError>),
    DeleteCancel,
}

pub fn update(app: &mut crate::app::App, message: Message) -> iced::Task<crate::app::Message>
{
    match message
    {
        Message::Refresh =>
        {
            app.plans.loading = true;
            app.plans.error = None;
            let api = app.api.clone();
            let phase_id = app.plans.phase_id;
            iced::Task::perform(
                async move {
                    let plans = api.plans(phase_id, None).await?;
                    let templates = api.templates(phase_id).await?;
                    let exercises = api.exercises(None).await?;
                    Ok((plans, templates, exercises))
                },
                |result| crate::app::Message::Plans(Message::Loaded(result)),
            )
        },
        Message::Loaded(result) => match result
        {
            Ok((plans, templates, exercises)) =>
            {
                app.plans.loading = false;
                app.plans.error = None;
                app.plans.plans = plans;
                app.plans.templates = templates;
                app.plans.exercises = exercises;
                iced::Task::none()
            },
            Err(error) =>
            {
                app.plans.loading = false;
                if matches!(error, crate::api::ApiError::Unauthorized)
                {
                    crate::app::report_error(app, error)
                }
                else
                {
                    app.plans.error = Some(error.to_string());
                    iced::Task::none()
                }
            },
        },
        Message::New =>
        {
            app.plan_form = crate::pages::plan_form::State::new(
                app.plans.phase_id,
                app.plans.phase_name.clone(),
                &app.plans.exercises,
                crate::platform::today(),
            );
            app.route = crate::app::Nav::PlanForm;
            iced::Task::none()
        },
        Message::Edit(id) =>
        {
            if let Some(plan) = app.plans.plans.iter().find(|plan| plan.id == id)
            {
                app.plan_form = crate::pages::plan_form::State::from_plan(
                    app.plans.phase_id,
                    app.plans.phase_name.clone(),
                    plan,
                    &app.plans.exercises,
                );
                app.route = crate::app::Nav::PlanForm;
            }
            iced::Task::none()
        },
        Message::FromTemplate(id) =>
        {
            if let Some(template) = app.plans.templates.iter().find(|item| item.id == id)
            {
                app.plan_form = crate::pages::plan_form::State::from_template(
                    app.plans.phase_id,
                    app.plans.phase_name.clone(),
                    template,
                    &app.plans.exercises,
                    crate::platform::today(),
                );
                app.route = crate::app::Nav::PlanForm;
            }
            iced::Task::none()
        },
        Message::Delete(id) =>
        {
            app.plans.confirm_delete = Some(id);
            iced::Task::none()
        },
        Message::DeleteConfirmed(id) =>
        {
            let api = app.api.clone();
            iced::Task::perform(async move { api.plan_delete(id).await }, |result| {
                crate::app::Message::Plans(Message::Deleted(result))
            })
        },
        Message::Deleted(result) => match result
        {
            Ok(_) =>
            {
                app.plans.confirm_delete = None;
                app.today.data = None;
                app.toast = Some(crate::copy::common(app.lang).saved.to_string());
                update(app, Message::Refresh)
            },
            Err(error) => crate::app::report_error(app, error),
        },
        Message::DeleteCancel =>
        {
            app.plans.confirm_delete = None;
            iced::Task::none()
        },
    }
}

pub fn view(app: &crate::app::App) -> crate::ui::El<'_>
{
    let text = crate::copy::plan::text(app.lang);
    let common = crate::copy::common(app.lang);
    let mut content = iced::widget::column![].spacing(12);
    content = content.push(
        iced::widget::text(app.plans.phase_name.as_str())
            .size(18)
            .font(crate::style::bold())
            .color(crate::style::TEXT),
    );
    content = content.push(crate::ui::primary_button(
        text.new_plan,
        crate::app::Message::Plans(Message::New),
    ));
    if app.plans.loading
    {
        content = content.push(crate::ui::loading(common.loading));
    }
    if let Some(error) = &app.plans.error
    {
        content = content.push(crate::ui::error_card(
            app.lang,
            error,
            Some(crate::ui::secondary_button(
                common.retry,
                crate::app::Message::Plans(Message::Refresh),
            )),
        ));
    }
    if !app.plans.templates.is_empty()
    {
        content = content.push(crate::ui::section_title(text.from_template));
        for template in &app.plans.templates
        {
            content = content.push(crate::ui::flat_card(
                iced::widget::row![
                    iced::widget::text(template.name.as_str())
                        .size(14)
                        .color(crate::style::TEXT)
                        .width(iced::Length::Fill),
                    crate::ui::secondary_button(
                        common.open,
                        crate::app::Message::Plans(Message::FromTemplate(template.id)),
                    ),
                ]
                .align_y(iced::Alignment::Center),
            ));
        }
    }
    if app.plans.plans.is_empty() && !app.plans.loading
    {
        content = content.push(crate::ui::empty_card(
            text.empty,
            text.no_templates,
            Some(crate::ui::secondary_button(
                text.new_plan,
                crate::app::Message::Plans(Message::New),
            )),
        ));
    }
    for plan in &app.plans.plans
    {
        let names = plan
            .items
            .iter()
            .map(|item| item.exercise_name.as_str())
            .collect::<Vec<_>>()
            .join(" / ");
        let mut card = iced::widget::column![
            iced::widget::row![
                iced::widget::text(plan.date.as_str())
                    .size(17)
                    .font(crate::style::bold())
                    .color(crate::style::TEXT),
                iced::widget::space().width(iced::Length::Fill),
                crate::ui::badge(
                    format!("{} {}", plan.items.len(), text.items),
                    crate::style::PRIMARY_LIGHT,
                ),
            ]
            .align_y(iced::Alignment::Center),
            iced::widget::text(
                if names.is_empty()
                {
                    text.no_items.to_string()
                }
                else
                {
                    names
                }
            )
            .size(13)
            .color(crate::style::MUTED),
            iced::widget::text(common.or_not_set(&plan.note))
                .size(13)
                .color(crate::style::TEXT),
            iced::widget::row![
                crate::ui::secondary_button(
                    common.edit,
                    crate::app::Message::Plans(Message::Edit(plan.id)),
                ),
                crate::ui::danger_button(
                    common.delete,
                    crate::app::Message::Plans(Message::Delete(plan.id)),
                ),
            ]
            .spacing(8),
        ]
        .spacing(8);
        if app.plans.confirm_delete == Some(plan.id)
        {
            card = card.push(
                iced::widget::row![
                    iced::widget::text(text.delete_confirm)
                        .size(12)
                        .color(crate::style::DANGER),
                    crate::ui::danger_button(
                        common.yes,
                        crate::app::Message::Plans(Message::DeleteConfirmed(plan.id)),
                    ),
                    crate::ui::secondary_button(
                        common.no,
                        crate::app::Message::Plans(Message::DeleteCancel),
                    ),
                ]
                .spacing(6),
            );
        }
        content = content.push(crate::ui::card(card));
    }
    crate::ui::detail_page(app, text.title, crate::app::Nav::Phases, content.into())
}
