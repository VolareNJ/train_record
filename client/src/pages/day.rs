#[derive(Debug, Clone, Default)]
pub struct State
{
    pub date: String,
    pub records: Vec<crate::model::DayRecord>,
    pub loading: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone)]
pub enum Message
{
    Refresh,
    Loaded(Result<Vec<crate::model::DayRecord>, crate::api::ApiError>),
}

pub fn update(app: &mut crate::app::App, message: Message) -> iced::Task<crate::app::Message>
{
    match message
    {
        Message::Refresh =>
        {
            app.day.loading = true;
            app.day.error = None;
            let api = app.api.clone();
            let date = app.day.date.clone();
            iced::Task::perform(async move { api.history_day(&date).await }, |result| {
                crate::app::Message::Day(Message::Loaded(result))
            })
        },
        Message::Loaded(result) => match result
        {
            Ok(records) =>
            {
                app.day.loading = false;
                app.day.error = None;
                app.day.records = records;
                iced::Task::none()
            },
            Err(error) =>
            {
                app.day.loading = false;
                if matches!(error, crate::api::ApiError::Unauthorized)
                {
                    crate::app::report_error(app, error)
                }
                else
                {
                    app.day.error = Some(error.to_string());
                    iced::Task::none()
                }
            },
        },
    }
}

pub fn view(app: &crate::app::App) -> crate::ui::El<'_>
{
    let text = crate::copy::history::text(app.lang);
    let common = crate::copy::common(app.lang);
    let mut content = iced::widget::column![].spacing(12);
    content = content.push(
        iced::widget::text(app.day.date.as_str())
            .size(18)
            .font(crate::style::bold())
            .color(crate::style::TEXT),
    );
    if app.day.loading
    {
        content = content.push(crate::ui::loading(common.loading));
    }
    if let Some(error) = &app.day.error
    {
        content = content.push(crate::ui::error_card(
            app.lang,
            error,
            Some(crate::ui::secondary_button(
                common.retry,
                crate::app::Message::Day(Message::Refresh),
            )),
        ));
    }
    if app.day.records.is_empty() && !app.day.loading
    {
        content = content.push(crate::ui::muted(text.no_records));
    }
    for record in &app.day.records
    {
        let body = iced::widget::column![
            iced::widget::row![
                iced::widget::text(record.exercise_name.as_str())
                    .size(17)
                    .font(crate::style::bold())
                    .color(crate::style::TEXT),
                iced::widget::space().width(iced::Length::Fill),
                crate::ui::badge(record.body_part.as_str(), crate::style::PRIMARY_LIGHT),
            ]
            .align_y(iced::Alignment::Center),
            iced::widget::text(common.volume(record.sets, record.reps, record.weight))
                .size(14)
                .color(crate::style::TEXT),
            iced::widget::text(format!(
                "1RM {:.1}  {} {}  {}",
                record.one_rm,
                common.rest,
                record.rest,
                crate::i18n::mode_label(app.lang, &record.mode)
            ))
            .size(13)
            .color(crate::style::MUTED),
            iced::widget::text(common.or_not_set(&record.feeling))
                .size(13)
                .color(crate::style::TEXT),
            iced::widget::text(common.or_not_set(&record.strategy))
                .size(13)
                .color(crate::style::WARN),
            iced::widget::text(common.or_not_set(&record.key_points))
                .size(12)
                .color(crate::style::MUTED),
        ]
        .spacing(5);
        content = content.push(
            iced::widget::button(body)
                .width(iced::Length::Fill)
                .padding(14)
                .style(crate::style::list_button)
                .on_press(crate::app::Message::OpenStats(
                    record.exercise_id,
                    record.exercise_name.clone(),
                )),
        );
    }
    crate::ui::detail_page(
        app,
        text.day_title,
        crate::app::Nav::Home(crate::app::Tab::History),
        content.into(),
    )
}
