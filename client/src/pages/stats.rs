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

    let metric = |label: &'static str, value: f64, color: iced::Color| {
        iced::widget::column![
            iced::widget::text(label)
                .size(12)
                .color(crate::style::MUTED),
            iced::widget::text(format!("{value:.1} {}", common.kg))
                .size(20)
                .font(crate::style::bold())
                .color(color),
        ]
        .spacing(3)
        .width(iced::Length::FillPortion(1))
    };
    content = content.push(crate::ui::card(
        iced::widget::column![
            iced::widget::row![
                metric(text.best, data.best_1rm, crate::style::PRIMARY_LIGHT),
                metric(text.best_2rm, data.best_2rm, crate::style::SUCCESS),
                metric(text.best_3rm, data.best_3rm, crate::style::WARN),
            ]
            .spacing(10),
            iced::widget::row![
                crate::ui::badge(
                    crate::i18n::part_label(app.lang, &data.exercise.body_part),
                    crate::style::PRIMARY_LIGHT,
                ),
                iced::widget::text(format!("{} {}", data.records.len(), text.record_count))
                    .size(12)
                    .color(crate::style::MUTED),
            ]
            .spacing(8)
            .align_y(iced::Alignment::Center),
            iced::widget::text(format!(
                "{}  {}kg  {}*{}  {}",
                crate::i18n::mode_label(app.lang, &data.exercise.default_mode),
                data.exercise.bar_weight,
                data.exercise.default_sets,
                data.exercise.default_reps,
                data.exercise.default_unit
            ))
            .size(12)
            .color(crate::style::MUTED),
        ]
        .spacing(10),
    ));

    content = content.push(crate::ui::section_title(text.trend));
    if data.records.is_empty()
    {
        content = content.push(crate::ui::muted(text.no_records));
    }
    else
    {
        content = content.push(crate::ui::flat_card(
            iced::widget::column![
                iced::widget::canvas::Canvas::new(TrendChart {
                    records: &data.records
                })
                .width(iced::Length::Fill)
                .height(180),
                iced::widget::row![
                    legend(text.weight_line, crate::style::MUTED),
                    legend(text.one_rm_line, crate::style::PRIMARY_LIGHT),
                    legend(text.two_rm_line, crate::style::SUCCESS),
                    iced::widget::space().width(iced::Length::Fill),
                    iced::widget::text(text.chart_hint)
                        .size(11)
                        .color(crate::style::MUTED),
                ]
                .spacing(12)
                .align_y(iced::Alignment::Center),
            ]
            .spacing(10),
        ));
    }

    content = content.push(crate::ui::section_title(text.recent));
    for record in data.records.iter().rev()
    {
        let observed = crate::weight::observed_label(
            &record.mode,
            record.weight,
            record.bar_weight,
            app.user.as_ref().and_then(|user| user.body_weight),
            &record.unit,
        );
        let mut card = iced::widget::column![
            iced::widget::row![
                iced::widget::text(record.date.as_str())
                    .size(13)
                    .color(crate::style::MUTED),
                iced::widget::space().width(iced::Length::Fill),
                crate::ui::badge(
                    if record.completed
                    {
                        common.completed
                    }
                    else
                    {
                        common.logged
                    },
                    if record.completed
                    {
                        crate::style::SUCCESS
                    }
                    else
                    {
                        crate::style::WARN
                    },
                ),
            ]
            .align_y(iced::Alignment::Center),
            iced::widget::text(observed)
                .size(15)
                .font(crate::style::bold())
                .color(crate::style::TEXT),
            iced::widget::text(format!(
                "{}  {} {}",
                common.volume(record.sets, record.reps, record.weight),
                common.rest,
                record.rest
            ))
            .size(12)
            .color(crate::style::MUTED),
            iced::widget::row![
                iced::widget::text(format!("1RM {:.1}", record.one_rm))
                    .size(12)
                    .color(crate::style::PRIMARY_LIGHT),
                iced::widget::text(format!("2RM {:.1}", record.two_rm))
                    .size(12)
                    .color(crate::style::SUCCESS),
                iced::widget::text(format!("3RM {:.1}", record.three_rm))
                    .size(12)
                    .color(crate::style::WARN),
            ]
            .spacing(12),
        ]
        .spacing(5);
        if !record.feeling.trim().is_empty()
        {
            card = card.push(
                iced::widget::text(format!("{}: {}", common.feeling, record.feeling))
                    .size(12)
                    .color(crate::style::TEXT),
            );
        }
        if !record.strategy.trim().is_empty()
        {
            card = card.push(
                iced::widget::text(format!("{}: {}", common.strategy, record.strategy))
                    .size(12)
                    .color(crate::style::WARN),
            );
        }
        content = content.push(crate::ui::flat_card(card));
    }

    crate::ui::detail_page(
        app,
        text.trend,
        crate::app::Nav::Home(crate::app::Tab::Library),
        content.into(),
    )
}

fn legend(label: &'static str, color: iced::Color) -> crate::ui::El<'static>
{
    iced::widget::row![
        iced::widget::container(iced::widget::space())
            .width(10)
            .height(10)
            .style(move |_| iced::widget::container::Style {
                background: Some(iced::Background::Color(color)),
                border: iced::Border {
                    radius: 5.0.into(),
                    ..iced::Border::default()
                },
                ..iced::widget::container::Style::default()
            }),
        iced::widget::text(label)
            .size(11)
            .color(crate::style::MUTED),
    ]
    .spacing(4)
    .align_y(iced::Alignment::Center)
    .into()
}

pub fn chart<'a>(records: &'a [crate::model::ExerciseRecord]) -> crate::ui::El<'a>
{
    iced::widget::canvas::Canvas::new(TrendChart { records })
        .width(iced::Length::Fill)
        .height(160)
        .into()
}

struct TrendChart<'a>
{
    records: &'a [crate::model::ExerciseRecord],
}

impl iced::widget::canvas::Program<crate::app::Message> for TrendChart<'_>
{
    type State = ();

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &iced::Renderer,
        _theme: &iced::Theme,
        bounds: iced::Rectangle,
        _cursor: iced::mouse::Cursor,
    ) -> Vec<iced::widget::canvas::Geometry>
    {
        let mut frame = iced::widget::canvas::Frame::new(renderer, bounds.size());
        let records = self
            .records
            .iter()
            .filter(|record| {
                crate::platform::days_since(&record.date)
                    .map(|days| days <= 180)
                    .unwrap_or(true)
            })
            .collect::<Vec<_>>();
        if records.is_empty()
        {
            return vec![frame.into_geometry()];
        }
        let pad = 10.0;
        let width = (bounds.width - pad * 2.0).max(1.0);
        let height = (bounds.height - pad * 2.0).max(1.0);
        let max = records
            .iter()
            .flat_map(|record| [record.weight, record.one_rm, record.two_rm])
            .fold(0.0_f64, f64::max)
            .max(1.0);
        let path = |get: fn(&crate::model::ExerciseRecord) -> f64| {
            let mut builder = iced::widget::canvas::path::Builder::new();
            let count = records.len();
            for (index, record) in records.iter().enumerate()
            {
                let x = if count <= 1
                {
                    pad + width / 2.0
                }
                else
                {
                    pad + width * index as f32 / (count - 1) as f32
                };
                let y = pad + height * (1.0 - (get(record) / max) as f32);
                let point = iced::Point::new(x, y);
                if index == 0
                {
                    builder.move_to(point);
                }
                else
                {
                    builder.line_to(point);
                }
            }
            builder.build()
        };
        for (get, color) in [
            (
                (|record: &crate::model::ExerciseRecord| record.weight)
                    as fn(&crate::model::ExerciseRecord) -> f64,
                crate::style::MUTED,
            ),
            (
                (|record: &crate::model::ExerciseRecord| record.one_rm)
                    as fn(&crate::model::ExerciseRecord) -> f64,
                crate::style::PRIMARY_LIGHT,
            ),
            (
                (|record: &crate::model::ExerciseRecord| record.two_rm)
                    as fn(&crate::model::ExerciseRecord) -> f64,
                crate::style::SUCCESS,
            ),
        ]
        {
            frame.stroke(
                &path(get),
                iced::widget::canvas::Stroke::default()
                    .with_color(color)
                    .with_width(2.0),
            );
        }
        vec![frame.into_geometry()]
    }
}
