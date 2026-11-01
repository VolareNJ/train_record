#[derive(Debug, Clone)]
pub struct State
{
    pub year: i32,
    pub month: u32,
    pub calendar: Option<crate::model::Calendar>,
    pub loading: bool,
    pub error: Option<String>,
    pub exercise_filter: Option<String>,
}

impl State
{
    pub fn new() -> Self
    {
        let (year, month) = crate::platform::current_month();
        Self {
            year,
            month,
            calendar: None,
            loading: false,
            error: None,
            exercise_filter: None,
        }
    }
}

#[derive(Debug, Clone)]
pub enum Message
{
    Refresh,
    Prev,
    Next,
    Year(i32),
    Filter(String),
    Loaded(Result<crate::model::Calendar, crate::api::ApiError>),
}

pub fn update(app: &mut crate::app::App, message: Message) -> iced::Task<crate::app::Message>
{
    match message
    {
        Message::Refresh =>
        {
            app.history.loading = true;
            app.history.error = None;
            let api = app.api.clone();
            let year = app.history.year;
            let month = app.history.month;
            iced::Task::perform(async move { api.calendar(year, month).await }, |result| {
                crate::app::Message::History(Message::Loaded(result))
            })
        },
        Message::Prev =>
        {
            let (year, month) =
                crate::platform::shift_month(app.history.year, app.history.month, -1);
            app.history.year = year;
            app.history.month = month;
            app.history.calendar = None;
            update(app, Message::Refresh)
        },
        Message::Next =>
        {
            let (year, month) =
                crate::platform::shift_month(app.history.year, app.history.month, 1);
            app.history.year = year;
            app.history.month = month;
            app.history.calendar = None;
            update(app, Message::Refresh)
        },
        Message::Year(year) =>
        {
            app.history.year = year;
            app.history.calendar = None;
            update(app, Message::Refresh)
        },
        Message::Filter(value) =>
        {
            let text = crate::copy::history::text(app.lang);
            app.history.exercise_filter = if value == text.all_parts
            {
                None
            }
            else
            {
                Some(crate::i18n::part_code(app.lang, &value))
            };
            iced::Task::none()
        },
        Message::Loaded(result) => match result
        {
            Ok(calendar) =>
            {
                app.history.loading = false;
                app.history.error = None;
                app.history.calendar = Some(calendar);
                iced::Task::none()
            },
            Err(error) =>
            {
                app.history.loading = false;
                if matches!(error, crate::api::ApiError::Unauthorized)
                {
                    crate::app::report_error(app, error)
                }
                else
                {
                    app.history.error = Some(error.to_string());
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
        iced::widget::row![
            crate::ui::ghost_button(text.prev, crate::app::Message::History(Message::Prev)),
            iced::widget::container(
                iced::widget::text(crate::platform::month_key(
                    app.history.year,
                    app.history.month
                ))
                .size(20)
                .font(crate::style::bold())
                .color(crate::style::TEXT),
            )
            .width(iced::Length::Fill)
            .center_x(iced::Length::Fill),
            crate::ui::ghost_button(text.next, crate::app::Message::History(Message::Next)),
        ]
        .width(iced::Length::Fill)
        .spacing(8)
        .align_y(iced::Alignment::Center),
    );

    let current_year = app.history.year;
    let years = (current_year.saturating_sub(5)..=current_year.saturating_add(1))
        .map(|year| year.to_string())
        .collect::<Vec<_>>();
    content = content.push(crate::ui::pick_field(
        text.year,
        years,
        Some(current_year.to_string()),
        move |value| {
            crate::app::Message::History(Message::Year(value.parse().unwrap_or(current_year)))
        },
    ));

    if app.history.loading
    {
        content = content.push(crate::ui::loading(common.loading));
    }
    if let Some(error) = &app.history.error
    {
        content = content.push(crate::ui::error_card(
            app.lang,
            error,
            Some(crate::ui::secondary_button(
                common.retry,
                crate::app::Message::History(Message::Refresh),
            )),
        ));
    }

    if let Some(calendar) = &app.history.calendar
    {
        let mut weekdays = iced::widget::row![].spacing(4);
        for label in text.weekdays
        {
            weekdays = weekdays.push(
                iced::widget::container(
                    iced::widget::text(label)
                        .size(12)
                        .color(crate::style::MUTED),
                )
                .width(iced::Length::FillPortion(1))
                .center_x(iced::Length::Fill),
            );
        }
        content = content.push(weekdays);

        let offset = crate::platform::weekday_index(app.history.year, app.history.month, 1);
        let days = crate::platform::days_in_month(app.history.year, app.history.month);
        let mut cells: Vec<(u32, bool)> = Vec::new();
        for _ in 0..offset
        {
            cells.push((0, false));
        }
        for day in 1..=days
        {
            let key = crate::platform::date_key(app.history.year, app.history.month, day);
            let trained = calendar.train_days.iter().any(|date| date == &key);
            cells.push((day, trained));
        }
        while !cells.len().is_multiple_of(7)
        {
            cells.push((0, false));
        }

        for week in cells.chunks(7)
        {
            let mut row = iced::widget::row![].spacing(4);
            for (day, trained) in week
            {
                if *day == 0
                {
                    row = row.push(
                        iced::widget::space()
                            .width(iced::Length::FillPortion(1))
                            .height(40),
                    );
                }
                else
                {
                    let key = crate::platform::date_key(app.history.year, app.history.month, *day);
                    if *trained
                    {
                        row = row.push(
                            iced::widget::button(
                                iced::widget::text(day.to_string())
                                    .size(14)
                                    .font(crate::style::regular()),
                            )
                            .width(iced::Length::FillPortion(1))
                            .height(40)
                            .padding(0)
                            .style(crate::style::primary_button)
                            .on_press(crate::app::Message::OpenDay(key)),
                        );
                    }
                    else
                    {
                        row = row.push(
                            iced::widget::container(
                                iced::widget::text(day.to_string())
                                    .size(14)
                                    .color(crate::style::MUTED),
                            )
                            .width(iced::Length::FillPortion(1))
                            .height(40)
                            .center_x(iced::Length::Fill)
                            .align_y(iced::Alignment::Center)
                            .style(crate::style::card_flat),
                        );
                    }
                }
            }
            content = content.push(row);
        }

        content = content.push(crate::ui::section_title(text.training_days));
        if calendar.train_days.is_empty()
        {
            content = content.push(crate::ui::muted(text.no_training));
        }
        else
        {
            let mut list = iced::widget::column![].spacing(6);
            for date in calendar.train_days.iter().rev()
            {
                list = list.push(
                    iced::widget::button(
                        iced::widget::text(date.as_str())
                            .size(14)
                            .color(crate::style::TEXT),
                    )
                    .width(iced::Length::Fill)
                    .padding([9, 12])
                    .style(crate::style::secondary_button)
                    .on_press(crate::app::Message::OpenDay(date.clone())),
                );
            }
            content = content.push(list);
        }
    }

    content = content.push(crate::ui::section_title(text.by_exercise));
    let mut parts = app
        .library
        .items
        .iter()
        .map(|exercise| exercise.body_part.clone())
        .collect::<Vec<_>>();
    parts.sort_by_key(|part| crate::weight::part_rank(part));
    parts.dedup();
    let mut options = vec![text.all_parts.to_string()];
    options.extend(
        parts
            .iter()
            .map(|part| crate::i18n::part_label(app.lang, part)),
    );
    let selected = app
        .history
        .exercise_filter
        .as_deref()
        .map(|part| crate::i18n::part_label(app.lang, part))
        .unwrap_or_else(|| text.all_parts.to_string());
    content = content.push(crate::ui::pick_field(
        text.by_exercise,
        options,
        Some(selected),
        |value| crate::app::Message::History(Message::Filter(value)),
    ));
    if app.library.items.is_empty()
    {
        content = content.push(crate::ui::muted(text.no_exercises));
    }
    else
    {
        for exercise in &app.library.items
        {
            if app
                .history
                .exercise_filter
                .as_deref()
                .is_some_and(|part| part != exercise.body_part)
            {
                continue;
            }
            content = content.push(
                iced::widget::button(
                    iced::widget::row![
                        iced::widget::column![
                            iced::widget::text(exercise.name.as_str())
                                .size(15)
                                .color(crate::style::TEXT),
                            iced::widget::text(crate::i18n::part_label(
                                app.lang,
                                &exercise.body_part
                            ))
                            .size(12)
                            .color(crate::style::MUTED),
                        ]
                        .spacing(3)
                        .width(iced::Length::Fill),
                        iced::widget::text(
                            exercise
                                .last_record_date
                                .as_deref()
                                .unwrap_or(common.not_set)
                        )
                        .size(12)
                        .color(crate::style::MUTED),
                    ]
                    .align_y(iced::Alignment::Center),
                )
                .width(iced::Length::Fill)
                .padding([9, 12])
                .style(crate::style::secondary_button)
                .on_press(crate::app::Message::OpenStats(
                    exercise.id,
                    exercise.name.clone(),
                )),
            );
        }
    }

    content.into()
}
