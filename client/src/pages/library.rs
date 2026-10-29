#[derive(Debug, Clone, Default)]
pub struct State
{
    pub items: Vec<crate::model::Exercise>,
    pub filter: String,
    pub loading: bool,
    pub error: Option<String>,
    pub confirm_delete: Option<i64>,
}

#[derive(Debug, Clone)]
pub enum Message
{
    Refresh,
    Filter(String),
    Loaded(Result<Vec<crate::model::Exercise>, crate::api::ApiError>),
    New,
    Edit(i64),
    Stats(i64),
    Delete(i64),
    DeleteConfirmed(i64),
    DeleteCancel,
    Deleted(Result<serde_json::Value, crate::api::ApiError>),
}

pub fn update(app: &mut crate::app::App, message: Message) -> iced::Task<crate::app::Message>
{
    match message
    {
        Message::Refresh =>
        {
            app.library.loading = true;
            app.library.error = None;
            let api = app.api.clone();
            let filter = if app.library.filter.is_empty()
            {
                None
            }
            else
            {
                Some(app.library.filter.clone())
            };
            iced::Task::perform(
                async move { api.exercises(filter.as_deref()).await },
                |result| crate::app::Message::Library(Message::Loaded(result)),
            )
        },
        Message::Filter(label) =>
        {
            app.library.filter = crate::i18n::part_code(app.lang, &label);
            update(app, Message::Refresh)
        },
        Message::Loaded(result) => match result
        {
            Ok(items) =>
            {
                app.library.loading = false;
                app.library.error = None;
                app.library.items = items;
                iced::Task::none()
            },
            Err(error) =>
            {
                app.library.loading = false;
                if matches!(error, crate::api::ApiError::Unauthorized)
                {
                    crate::app::report_error(app, error)
                }
                else
                {
                    app.library.error = Some(error.to_string());
                    iced::Task::none()
                }
            },
        },
        Message::New =>
        {
            app.exercise_form = crate::pages::exercise_form::State::new();
            app.route = crate::app::Nav::ExerciseForm;
            iced::Task::none()
        },
        Message::Edit(id) =>
        {
            if let Some(exercise) = app.library.items.iter().find(|item| item.id == id)
            {
                app.exercise_form = crate::pages::exercise_form::State::from_exercise(exercise);
                app.route = crate::app::Nav::ExerciseForm;
            }
            iced::Task::none()
        },
        Message::Stats(id) =>
        {
            let name = app
                .library
                .items
                .iter()
                .find(|item| item.id == id)
                .map(|item| item.name.clone())
                .unwrap_or_default();
            crate::app::open_stats(app, id, name)
        },
        Message::Delete(id) =>
        {
            app.library.confirm_delete = Some(id);
            iced::Task::none()
        },
        Message::DeleteConfirmed(id) =>
        {
            let api = app.api.clone();
            iced::Task::perform(async move { api.exercise_delete(id).await }, |result| {
                crate::app::Message::Library(Message::Deleted(result))
            })
        },
        Message::DeleteCancel =>
        {
            app.library.confirm_delete = None;
            iced::Task::none()
        },
        Message::Deleted(result) => match result
        {
            Ok(_) =>
            {
                app.library.confirm_delete = None;
                app.toast = Some(crate::copy::common(app.lang).saved.to_string());
                update(app, Message::Refresh)
            },
            Err(error) => crate::app::report_error(app, error),
        },
    }
}

pub fn view(app: &crate::app::App) -> crate::ui::El<'_>
{
    let text = crate::copy::library::text(app.lang);
    let common = crate::copy::common(app.lang);
    let mut content = iced::widget::column![].spacing(12);
    content = content.push(crate::ui::primary_button(
        text.new_exercise,
        crate::app::Message::Library(Message::New),
    ));

    let filter_options: Vec<String> = text
        .body_parts
        .iter()
        .map(|part| (*part).to_string())
        .collect();
    let filter_selected = if app.library.filter.is_empty()
    {
        Some(text.body_parts[0].to_string())
    }
    else
    {
        Some(crate::i18n::part_label(app.lang, &app.library.filter))
    };
    content = content.push(crate::ui::pick_field(
        text.filter,
        filter_options,
        filter_selected,
        |label| crate::app::Message::Library(Message::Filter(label)),
    ));

    if app.library.loading
    {
        content = content.push(crate::ui::loading(common.loading));
    }
    if let Some(error) = &app.library.error
    {
        content = content.push(crate::ui::error_card(
            app.lang,
            error,
            Some(crate::ui::secondary_button(
                common.retry,
                crate::app::Message::Library(Message::Refresh),
            )),
        ));
    }
    if app.library.items.is_empty() && !app.library.loading
    {
        content = content.push(crate::ui::empty_card(
            text.empty,
            text.tip,
            Some(crate::ui::secondary_button(
                text.new_exercise,
                crate::app::Message::Library(Message::New),
            )),
        ));
    }

    for exercise in &app.library.items
    {
        let best = exercise
            .best_1rm
            .map(|value| format!("{value:.1} {}", common.kg))
            .unwrap_or_else(|| common.not_set.to_string());
        let last = exercise
            .last_record_date
            .as_deref()
            .unwrap_or(common.not_set);
        let part = crate::i18n::part_label(app.lang, &exercise.body_part);
        let mut card = iced::widget::column![
            iced::widget::row![
                iced::widget::text(exercise.name.as_str())
                    .size(17)
                    .font(crate::style::bold())
                    .color(crate::style::TEXT),
                iced::widget::space().width(iced::Length::Fill),
                crate::ui::badge(part, crate::style::PRIMARY_LIGHT),
            ]
            .align_y(iced::Alignment::Center),
            crate::ui::stat_line(text.best, best),
            crate::ui::stat_line(text.last, last),
            iced::widget::row![
                crate::ui::secondary_button(
                    text.stats,
                    crate::app::Message::Library(Message::Stats(exercise.id)),
                ),
                crate::ui::secondary_button(
                    common.edit,
                    crate::app::Message::Library(Message::Edit(exercise.id)),
                ),
                crate::ui::danger_button(
                    common.delete,
                    crate::app::Message::Library(Message::Delete(exercise.id)),
                ),
            ]
            .spacing(8),
        ]
        .spacing(8);
        if app.library.confirm_delete == Some(exercise.id)
        {
            card = card.push(
                iced::widget::row![
                    iced::widget::text(text.delete_confirm)
                        .size(12)
                        .color(crate::style::DANGER),
                    crate::ui::danger_button(
                        common.yes,
                        crate::app::Message::Library(Message::DeleteConfirmed(exercise.id)),
                    ),
                    crate::ui::secondary_button(
                        common.no,
                        crate::app::Message::Library(Message::DeleteCancel),
                    ),
                ]
                .spacing(6)
                .align_y(iced::Alignment::Center),
            );
        }
        content = content.push(crate::ui::card(card));
    }

    content.into()
}
