#[derive(Debug, Clone)]
pub struct State
{
    pub year: i32,
    pub month: u32,
    pub calendar: Option<crate::model::Calendar>,
    pub loading: bool,
    pub error: Option<String>,
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
        }
    }
}

#[derive(Debug, Clone)]
pub enum Message
{
    Refresh,
    Prev,
    Next,
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
            iced::widget::text(crate::platform::month_key(
                app.history.year,
                app.history.month
            ))
            .size(20)
            .font(crate::style::bold())
            .color(crate::style::TEXT),
            crate::ui::ghost_button(text.next, crate::app::Message::History(Message::Next)),
        ]
        .spacing(8)
        .align_y(iced::Alignment::Center),
    );

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
                    let style = if *trained
                    {
                        crate::style::primary_button
                    }
                    else
                    {
                        crate::style::secondary_button
                    };
                    row = row.push(
                        iced::widget::button(
                            iced::widget::text(day.to_string())
                                .size(14)
                                .font(crate::style::regular()),
                        )
                        .width(iced::Length::FillPortion(1))
                        .height(40)
                        .padding(0)
                        .style(style)
                        .on_press(crate::app::Message::OpenDay(key)),
                    );
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

    content.into()
}
