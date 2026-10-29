#[derive(Debug, Clone, Default)]
pub struct State
{
    pub phase_id: i64,
    pub phase_name: String,
    pub items: Vec<crate::model::Template>,
    pub exercises: Vec<crate::model::Exercise>,
    pub loading: bool,
    pub error: Option<String>,
    pub confirm_delete: Option<i64>,
}

#[derive(Debug, Clone)]
pub enum Message
{
    Refresh,
    Loaded(
        Result<(Vec<crate::model::Template>, Vec<crate::model::Exercise>), crate::api::ApiError>,
    ),
    New,
    Edit(i64),
    Delete(i64),
    DeleteConfirmed(i64),
    Deleted(Result<serde_json::Value, crate::api::ApiError>),
    DeleteCancel,
    UseTemplate(i64),
}

pub fn update(app: &mut crate::app::App, message: Message) -> iced::Task<crate::app::Message>
{
    match message
    {
        Message::Refresh =>
        {
            app.templates.loading = true;
            app.templates.error = None;
            let api = app.api.clone();
            let phase_id = app.templates.phase_id;
            iced::Task::perform(
                async move {
                    let templates = api.templates(phase_id).await?;
                    let exercises = api.exercises(None).await?;
                    Ok((templates, exercises))
                },
                |result| crate::app::Message::Templates(Message::Loaded(result)),
            )
        },
        Message::Loaded(result) => match result
        {
            Ok((templates, exercises)) =>
            {
                app.templates.loading = false;
                app.templates.error = None;
                app.templates.items = templates;
                app.templates.exercises = exercises;
                iced::Task::none()
            },
            Err(error) =>
            {
                app.templates.loading = false;
                if matches!(error, crate::api::ApiError::Unauthorized)
                {
                    crate::app::report_error(app, error)
                }
                else
                {
                    app.templates.error = Some(error.to_string());
                    iced::Task::none()
                }
            },
        },
        Message::New =>
        {
            app.template_form = crate::pages::template_form::State::new(
                app.templates.phase_id,
                app.templates.phase_name.clone(),
                &app.templates.exercises,
            );
            app.route = crate::app::Nav::TemplateForm;
            iced::Task::none()
        },
        Message::Edit(id) =>
        {
            if let Some(template) = app.templates.items.iter().find(|item| item.id == id)
            {
                app.template_form = crate::pages::template_form::State::from_template(
                    app.templates.phase_id,
                    app.templates.phase_name.clone(),
                    template,
                    &app.templates.exercises,
                );
                app.route = crate::app::Nav::TemplateForm;
            }
            iced::Task::none()
        },
        Message::Delete(id) =>
        {
            app.templates.confirm_delete = Some(id);
            iced::Task::none()
        },
        Message::DeleteConfirmed(id) =>
        {
            let api = app.api.clone();
            iced::Task::perform(async move { api.template_delete(id).await }, |result| {
                crate::app::Message::Templates(Message::Deleted(result))
            })
        },
        Message::Deleted(result) => match result
        {
            Ok(_) =>
            {
                app.templates.confirm_delete = None;
                app.toast = Some(crate::copy::common(app.lang).saved.to_string());
                update(app, Message::Refresh)
            },
            Err(error) => crate::app::report_error(app, error),
        },
        Message::DeleteCancel =>
        {
            app.templates.confirm_delete = None;
            iced::Task::none()
        },
        Message::UseTemplate(id) =>
        {
            if let Some(template) = app.templates.items.iter().find(|item| item.id == id)
            {
                app.plans.phase_id = app.templates.phase_id;
                app.plans.phase_name = app.templates.phase_name.clone();
                app.plans.plans.clear();
                app.plans.templates.clear();
                app.plan_form = crate::pages::plan_form::State::from_template(
                    app.templates.phase_id,
                    app.templates.phase_name.clone(),
                    template,
                    &app.templates.exercises,
                    crate::platform::today(),
                );
                app.route = crate::app::Nav::PlanForm;
            }
            iced::Task::none()
        },
    }
}

pub fn view(app: &crate::app::App) -> crate::ui::El<'_>
{
    let text = crate::copy::template::text(app.lang);
    let common = crate::copy::common(app.lang);
    let mut content = iced::widget::column![].spacing(12);
    content = content.push(
        iced::widget::text(app.templates.phase_name.as_str())
            .size(18)
            .font(crate::style::bold())
            .color(crate::style::TEXT),
    );
    content = content.push(crate::ui::primary_button(
        text.new_template,
        crate::app::Message::Templates(Message::New),
    ));
    if app.templates.loading
    {
        content = content.push(crate::ui::loading(common.loading));
    }
    if let Some(error) = &app.templates.error
    {
        content = content.push(crate::ui::error_card(
            app.lang,
            error,
            Some(crate::ui::secondary_button(
                common.retry,
                crate::app::Message::Templates(Message::Refresh),
            )),
        ));
    }
    if app.templates.items.is_empty() && !app.templates.loading
    {
        content = content.push(crate::ui::empty_card(
            text.empty,
            text.add_hint,
            Some(crate::ui::secondary_button(
                text.new_template,
                crate::app::Message::Templates(Message::New),
            )),
        ));
    }
    for template in &app.templates.items
    {
        let names = template
            .items
            .iter()
            .map(|item| item.exercise_name.as_str())
            .collect::<Vec<_>>()
            .join(" / ");
        let mut card = iced::widget::column![
            iced::widget::row![
                iced::widget::text(template.name.as_str())
                    .size(17)
                    .font(crate::style::bold())
                    .color(crate::style::TEXT),
                iced::widget::space().width(iced::Length::Fill),
                crate::ui::badge(
                    format!("{} {}", template.items.len(), text.items),
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
            iced::widget::row![
                crate::ui::secondary_button(
                    text.use_template,
                    crate::app::Message::Templates(Message::UseTemplate(template.id)),
                ),
                crate::ui::secondary_button(
                    common.edit,
                    crate::app::Message::Templates(Message::Edit(template.id)),
                ),
                crate::ui::danger_button(
                    common.delete,
                    crate::app::Message::Templates(Message::Delete(template.id)),
                ),
            ]
            .spacing(8),
        ]
        .spacing(8);
        if app.templates.confirm_delete == Some(template.id)
        {
            card = card.push(
                iced::widget::row![
                    iced::widget::text(text.delete_confirm)
                        .size(12)
                        .color(crate::style::DANGER),
                    crate::ui::danger_button(
                        common.yes,
                        crate::app::Message::Templates(Message::DeleteConfirmed(template.id)),
                    ),
                    crate::ui::secondary_button(
                        common.no,
                        crate::app::Message::Templates(Message::DeleteCancel),
                    ),
                ]
                .spacing(6),
            );
        }
        content = content.push(crate::ui::card(card));
    }
    crate::ui::detail_page(app, text.title, crate::app::Nav::Phases, content.into())
}
