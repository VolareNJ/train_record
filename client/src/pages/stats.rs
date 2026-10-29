#[derive(Debug, Clone, Default)]
pub struct State
{
    pub exercise_id: i64,
    pub name: String,
    pub data: Option<crate::model::ExerciseStats>,
    pub loading: bool,
    pub error: Option<String>,
}

impl State
{
    pub fn new(exercise_id: i64, name: String) -> Self
    {
        Self {
            exercise_id,
            name,
            data: None,
            loading: false,
            error: None,
        }
    }
}

#[derive(Debug, Clone)]
pub enum Message
{
    Refresh,
    Loaded(Result<crate::model::ExerciseStats, crate::api::ApiError>),
}

pub fn update(app: &mut crate::app::App, message: Message) -> iced::Task<crate::app::Message>
{
    match message
    {
        Message::Refresh =>
        {
            app.stats.loading = true;
            app.stats.error = None;
            let api = app.api.clone();
            let id = app.stats.exercise_id;
            iced::Task::perform(async move { api.exercise_stats(id).await }, |result| {
                crate::app::Message::Stats(Message::Loaded(result))
            })
        },
        Message::Loaded(result) => match result
        {
            Ok(data) =>
            {
                app.stats.loading = false;
                app.stats.error = None;
                app.stats.data = Some(data);
                iced::Task::none()
            },
            Err(error) =>
            {
                app.stats.loading = false;
                if matches!(error, crate::api::ApiError::Unauthorized)
                {
                    crate::app::report_error(app, error)
                }
                else
                {
                    app.stats.error = Some(error.to_string());
                    iced::Task::none()
                }
            },
        },
    }
}

pub fn view(app: &crate::app::App) -> crate::ui::El<'_>
{
    let text = crate::copy::stats::text(app.lang);
    let common = crate::copy::common(app.lang);
    let mut content = iced::widget::column![].spacing(12);
    let title = app
        .stats
        .data
        .as_ref()
        .map(|data| data.exercise.name.as_str())
        .unwrap_or(app.stats.name.as_str());
    content = content.push(
        iced::widget::text(title)
            .size(20)
            .font(crate::style::bold())
            .color(crate::style::TEXT),
    );
    if app.stats.loading
    {
        content = content.push(crate::ui::loading(common.loading));
    }
    if let Some(error) = &app.stats.error
    {
        content = content.push(crate::ui::error_card(
            app.lang,
            error,
            Some(crate::ui::secondary_button(
                common.retry,
                crate::app::Message::Stats(Message::Refresh),
            )),
        ));
    }
    let Some(data) = &app.stats.data
    else
    {
        return crate::ui::detail_page(
            app,
            text.trend,
            crate::app::Nav::Home(crate::app::Tab::Library),
            content.into(),
        );
    };

    content = content.push(crate::ui::card(
        iced::widget::row![
            iced::widget::column![
                iced::widget::text(text.best)
                    .size(13)
                    .color(crate::style::MUTED),
                iced::widget::text(format!("{:.1} {}", data.best_1rm, common.kg))
                    .size(28)
                    .font(crate::style::bold())
                    .color(crate::style::PRIMARY_LIGHT),
            ]
            .spacing(4),
            iced::widget::space().width(iced::Length::Fill),
            iced::widget::column![
                iced::widget::text(crate::i18n::part_label(app.lang, &data.exercise.body_part))
                    .size(13)
                    .color(crate::style::MUTED),
                iced::widget::text(format!("{} {}", data.records.len(), text.record_count))
                    .size(13)
                    .color(crate::style::TEXT),
            ]
            .spacing(4),
        ]
        .align_y(iced::Alignment::Center),
    ));

    content = content.push(crate::ui::section_title(text.trend));
    if data.records.is_empty()
    {
        content = content.push(crate::ui::muted(text.no_records));
    }
    else
    {
        let start = data.records.len().saturating_sub(12);
        let recent = &data.records[start..];
        let max = recent
            .iter()
            .map(|record| record.one_rm)
            .fold(0.0_f64, f64::max)
            .max(1.0);
        let mut bars = iced::widget::row![]
            .spacing(4)
            .align_y(iced::alignment::Vertical::Bottom);
        for record in recent
        {
            let height = (record.one_rm / max * 110.0).max(4.0) as f32;
            let label = record
                .date
                .get(5..)
                .unwrap_or(record.date.as_str())
                .to_string();
            bars = bars.push(
                iced::widget::column![
                    iced::widget::container(
                        iced::widget::container(iced::widget::space())
                            .width(iced::Length::Fill)
                            .height(height)
                            .style(|_| iced::widget::container::Style {
                                background: Some(iced::Background::Color(crate::style::PRIMARY)),
                                border: iced::Border {
                                    radius: 4.0.into(),
                                    ..iced::Border::default()
                                },
                                ..iced::widget::container::Style::default()
                            }),
                    )
                    .width(iced::Length::Fill)
                    .height(110)
                    .align_y(iced::alignment::Vertical::Bottom),
                    iced::widget::text(label).size(9).color(crate::style::MUTED),
                ]
                .width(iced::Length::Fixed(26.0))
                .spacing(4),
            );
        }
        content = content.push(crate::ui::flat_card(bars));
    }

    content = content.push(crate::ui::section_title(text.recent));
    for record in data.records.iter().rev()
    {
        content = content.push(crate::ui::flat_card(
            iced::widget::row![
                iced::widget::column![
                    iced::widget::text(record.date.as_str())
                        .size(13)
                        .color(crate::style::MUTED),
                    iced::widget::text(common.volume(record.sets, record.reps, record.weight))
                        .size(14)
                        .color(crate::style::TEXT),
                ]
                .spacing(4)
                .width(iced::Length::Fill),
                iced::widget::text(format!("{:.1} {}", record.one_rm, common.kg))
                    .size(15)
                    .font(crate::style::bold())
                    .color(crate::style::PRIMARY_LIGHT),
            ]
            .align_y(iced::Alignment::Center),
        ));
    }

    crate::ui::detail_page(
        app,
        text.trend,
        crate::app::Nav::Home(crate::app::Tab::Library),
        content.into(),
    )
}
