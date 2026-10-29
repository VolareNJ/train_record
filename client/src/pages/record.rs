#[derive(Debug, Clone, Default)]
pub struct State
{
    pub plan_id: i64,
    pub item_id: i64,
    pub exercise_name: String,
    pub body_part: String,
    pub plan_sets: Option<i64>,
    pub plan_reps: Option<i64>,
    pub plan_weight: Option<f64>,
    pub plan_rest: Option<i64>,
    pub plan_key_points: Option<String>,
    pub last: Option<crate::model::LastRecord>,
    pub weight: String,
    pub sets: String,
    pub reps: String,
    pub rest: String,
    pub feeling: String,
    pub strategy: String,
    pub key_points: String,
    pub completed: bool,
    pub mode: String,
    pub converter_value: String,
    pub bar_weight: String,
    pub saving: bool,
    pub error: Option<String>,
    pub confirm_delete: bool,
}

impl State
{
    pub fn from_item(app: &crate::app::App, plan_id: i64, item: &crate::model::TodayItem) -> Self
    {
        let last = item.last_record.clone();
        let weight = last
            .as_ref()
            .map(|record| format!("{:.1}", record.weight))
            .or_else(|| item.plan_weight.map(|weight| format!("{weight:.1}")))
            .unwrap_or_default();
        let sets = last
            .as_ref()
            .map(|record| record.sets)
            .or(item.plan_sets)
            .unwrap_or(3);
        let reps = last
            .as_ref()
            .map(|record| record.reps)
            .or(item.plan_reps)
            .unwrap_or(8);
        let rest = last
            .as_ref()
            .map(|record| record.rest)
            .or(item.plan_rest)
            .unwrap_or(0);
        let _ = app;
        Self {
            plan_id,
            item_id: item.id,
            exercise_name: item.exercise_name.clone(),
            body_part: item.body_part.clone(),
            plan_sets: item.plan_sets,
            plan_reps: item.plan_reps,
            plan_weight: item.plan_weight,
            plan_rest: item.plan_rest,
            plan_key_points: item.plan_key_points.clone(),
            last,
            weight,
            sets: sets.to_string(),
            reps: reps.to_string(),
            rest: rest.to_string(),
            feeling: String::new(),
            strategy: String::new(),
            key_points: item.plan_key_points.clone().unwrap_or_default(),
            completed: item.last_record.is_some(),
            mode: "bar".to_string(),
            converter_value: String::new(),
            bar_weight: "20".to_string(),
            saving: false,
            error: None,
            confirm_delete: false,
        }
    }

    fn total(&self, body_weight: Option<f64>) -> Option<f64>
    {
        let raw = self.converter_value.trim().parse::<f64>().ok()?;
        match self.mode.as_str()
        {
            "bar" => self
                .bar_weight
                .trim()
                .parse::<f64>()
                .ok()
                .map(|bar| raw * 2.0 + bar),
            "std" => Some(raw),
            "support" => body_weight.map(|weight| weight - raw),
            "lb" => Some(raw / 2.2046),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub enum Message
{
    Weight(String),
    Sets(String),
    Reps(String),
    Rest(String),
    Feeling(String),
    Strategy(String),
    KeyPoints(String),
    Completed(bool),
    Mode(String),
    ConverterValue(String),
    BarWeight(String),
    FillWeight,
    Save,
    Saved(Result<serde_json::Value, crate::api::ApiError>),
    Delete,
    DeleteConfirm,
    DeleteCancel,
    Deleted(Result<serde_json::Value, crate::api::ApiError>),
}

pub fn update(app: &mut crate::app::App, message: Message) -> iced::Task<crate::app::Message>
{
    let text = crate::copy::record::text(app.lang);
    match message
    {
        Message::Weight(value) =>
        {
            app.record.weight = value;
            iced::Task::none()
        },
        Message::Sets(value) =>
        {
            app.record.sets = value;
            iced::Task::none()
        },
        Message::Reps(value) =>
        {
            app.record.reps = value;
            iced::Task::none()
        },
        Message::Rest(value) =>
        {
            app.record.rest = value;
            iced::Task::none()
        },
        Message::Feeling(value) =>
        {
            app.record.feeling = value;
            iced::Task::none()
        },
        Message::Strategy(value) =>
        {
            app.record.strategy = value;
            iced::Task::none()
        },
        Message::KeyPoints(value) =>
        {
            app.record.key_points = value;
            iced::Task::none()
        },
        Message::Completed(value) =>
        {
            app.record.completed = value;
            iced::Task::none()
        },
        Message::Mode(value) =>
        {
            app.record.mode = value;
            iced::Task::none()
        },
        Message::ConverterValue(value) =>
        {
            app.record.converter_value = value;
            iced::Task::none()
        },
        Message::BarWeight(value) =>
        {
            app.record.bar_weight = value;
            iced::Task::none()
        },
        Message::FillWeight =>
        {
            let body_weight = app.user.as_ref().and_then(|user| user.body_weight);
            match app.record.total(body_weight)
            {
                Some(total) =>
                {
                    app.record.weight = format!("{total:.1}");
                    app.record.error = None;
                },
                None =>
                {
                    app.record.error = Some(
                        if app.record.mode == "support" && body_weight.is_none()
                        {
                            text.support_needs_body_weight.to_string()
                        }
                        else
                        {
                            text.invalid_weight.to_string()
                        },
                    );
                },
            }
            iced::Task::none()
        },
        Message::Save =>
        {
            let weight = app.record.weight.trim().parse::<f64>();
            let sets = app.record.sets.trim().parse::<i64>();
            let reps = app.record.reps.trim().parse::<i64>();
            let rest = app.record.rest.trim().parse::<i64>();
            let (Ok(weight), Ok(sets), Ok(reps), Ok(rest)) = (weight, sets, reps, rest)
            else
            {
                app.record.error = Some(
                    if app.record.weight.trim().parse::<f64>().is_err()
                    {
                        text.invalid_weight.to_string()
                    }
                    else
                    {
                        text.invalid_sets.to_string()
                    },
                );
                return iced::Task::none();
            };
            if weight < 0.0 || sets < 0 || reps < 0 || rest < 0
            {
                app.record.error = Some(text.invalid_sets.to_string());
                return iced::Task::none();
            }
            let input = crate::model::RecordInput {
                weight,
                sets,
                reps,
                rest,
                feeling: app.record.feeling.clone(),
                strategy: app.record.strategy.clone(),
                key_points: app.record.key_points.clone(),
                completed: app.record.completed,
            };
            let api = app.api.clone();
            let plan_id = app.record.plan_id;
            let item_id = app.record.item_id;
            app.record.saving = true;
            app.record.error = None;
            iced::Task::perform(
                async move { api.record_upsert(plan_id, item_id, &input).await },
                |result| crate::app::Message::Record(Message::Saved(result)),
            )
        },
        Message::Saved(result) => match result
        {
            Ok(_) =>
            {
                app.record.saving = false;
                app.toast = Some(crate::copy::common(app.lang).saved.to_string());
                app.route = crate::app::Nav::Home(crate::app::Tab::Today);
                crate::pages::today::update(app, crate::pages::today::Message::Refresh)
            },
            Err(error) =>
            {
                app.record.saving = false;
                if matches!(error, crate::api::ApiError::Unauthorized)
                {
                    crate::app::report_error(app, error)
                }
                else
                {
                    app.record.error = Some(error.to_string());
                    iced::Task::none()
                }
            },
        },
        Message::Delete =>
        {
            let Some(last) = &app.record.last
            else
            {
                return iced::Task::none();
            };
            let id = last.id;
            let api = app.api.clone();
            iced::Task::perform(async move { api.record_delete(id).await }, |result| {
                crate::app::Message::Record(Message::Deleted(result))
            })
        },
        Message::DeleteConfirm =>
        {
            app.record.confirm_delete = true;
            iced::Task::none()
        },
        Message::DeleteCancel =>
        {
            app.record.confirm_delete = false;
            iced::Task::none()
        },
        Message::Deleted(result) => match result
        {
            Ok(_) =>
            {
                app.record.confirm_delete = false;
                app.toast = Some(crate::copy::common(app.lang).saved.to_string());
                app.route = crate::app::Nav::Home(crate::app::Tab::Today);
                crate::pages::today::update(app, crate::pages::today::Message::Refresh)
            },
            Err(error) => crate::app::report_error(app, error),
        },
    }
}

fn mode_options(text: &crate::copy::record::Text) -> Vec<String>
{
    vec![
        text.mode_bar.to_string(),
        text.mode_std.to_string(),
        text.mode_support.to_string(),
        text.mode_lb.to_string(),
    ]
}

fn mode_label(text: &crate::copy::record::Text, mode: &str) -> String
{
    match mode
    {
        "std" => text.mode_std.to_string(),
        "support" => text.mode_support.to_string(),
        "lb" => text.mode_lb.to_string(),
        _ => text.mode_bar.to_string(),
    }
}

fn mode_from_label(text: &crate::copy::record::Text, label: &str) -> String
{
    if label == text.mode_std
    {
        "std".to_string()
    }
    else if label == text.mode_support
    {
        "support".to_string()
    }
    else if label == text.mode_lb
    {
        "lb".to_string()
    }
    else
    {
        "bar".to_string()
    }
}

pub fn view(app: &crate::app::App) -> crate::ui::El<'_>
{
    let text = crate::copy::record::text(app.lang);
    let common = crate::copy::common(app.lang);
    let state = &app.record;
    let mut content = iced::widget::column![].spacing(12);

    let rest = format!("{} {}", common.rest, common.number_text(state.plan_rest));
    let plan_line = format!(
        "{}  {}  {}  {}",
        common.sets_reps(state.plan_sets, state.plan_reps),
        common.weight_text(state.plan_weight),
        rest,
        state.body_part
    );
    content = content.push(crate::ui::card(
        iced::widget::column![
            iced::widget::text(text.plan_values)
                .size(13)
                .color(crate::style::MUTED),
            iced::widget::text(plan_line)
                .size(15)
                .color(crate::style::TEXT),
            iced::widget::text(state.plan_key_points.as_deref().unwrap_or(common.not_set))
                .size(13)
                .color(crate::style::MUTED),
        ]
        .spacing(6),
    ));

    if let Some(last) = &state.last
    {
        content = content.push(crate::ui::flat_card(
            iced::widget::column![
                iced::widget::text(text.last_values)
                    .size(13)
                    .color(crate::style::MUTED),
                iced::widget::text(common.volume(last.sets, last.reps, last.weight))
                    .size(15)
                    .color(crate::style::TEXT),
                iced::widget::text(common.or_not_set(&last.feeling))
                    .size(13)
                    .color(crate::style::MUTED),
                iced::widget::text(common.or_not_set(&last.strategy))
                    .size(13)
                    .color(crate::style::WARN),
            ]
            .spacing(5),
        ));
    }

    let form = iced::widget::column![
        crate::ui::section_title(text.actual_values),
        crate::ui::text_field(
            common.weight,
            "0.0",
            &state.weight,
            |value| crate::app::Message::Record(Message::Weight(value)),
            false,
        ),
        iced::widget::row![
            crate::ui::text_field(
                common.sets,
                "3",
                &state.sets,
                |value| crate::app::Message::Record(Message::Sets(value)),
                false,
            )
            .width(iced::Length::Fill),
            crate::ui::text_field(
                common.reps,
                "8",
                &state.reps,
                |value| crate::app::Message::Record(Message::Reps(value)),
                false,
            )
            .width(iced::Length::Fill),
            crate::ui::text_field(
                common.rest,
                "90",
                &state.rest,
                |value| crate::app::Message::Record(Message::Rest(value)),
                false,
            )
            .width(iced::Length::Fill),
        ]
        .spacing(10),
        crate::ui::text_field(
            common.feeling,
            text.feeling_hint,
            &state.feeling,
            |value| crate::app::Message::Record(Message::Feeling(value)),
            false,
        ),
        crate::ui::text_field(
            common.strategy,
            text.strategy_hint,
            &state.strategy,
            |value| crate::app::Message::Record(Message::Strategy(value)),
            false,
        ),
        crate::ui::text_field(
            common.key_points,
            text.key_points_hint,
            &state.key_points,
            |value| crate::app::Message::Record(Message::KeyPoints(value)),
            false,
        ),
        crate::ui::checkbox_field(common.completed, state.completed, |value| {
            crate::app::Message::Record(Message::Completed(value))
        },),
    ]
    .spacing(10);
    content = content.push(crate::ui::card(form));

    let total = state.total(app.user.as_ref().and_then(|user| user.body_weight));
    let total_text = total
        .map(|value| format!("{value:.1} {}", common.kg))
        .unwrap_or_else(|| common.not_set.to_string());
    let mut converter = iced::widget::column![
        crate::ui::section_title(text.converter),
        crate::ui::pick_field(
            text.mode,
            mode_options(&text),
            Some(mode_label(&text, &state.mode)),
            move |label| crate::app::Message::Record(Message::Mode(mode_from_label(&text, &label))),
        ),
        crate::ui::text_field(
            text.raw_value,
            "0.0",
            &state.converter_value,
            |value| crate::app::Message::Record(Message::ConverterValue(value)),
            false,
        ),
    ]
    .spacing(10);
    if state.mode == "bar"
    {
        converter = converter.push(crate::ui::text_field(
            text.bar_weight,
            "20",
            &state.bar_weight,
            |value| crate::app::Message::Record(Message::BarWeight(value)),
            false,
        ));
    }
    converter = converter
        .push(
            iced::widget::text(format!("{}: {}", text.total, total_text))
                .size(18)
                .font(crate::style::bold())
                .color(crate::style::PRIMARY_LIGHT),
        )
        .push(crate::ui::secondary_button(
            text.fill_weight,
            crate::app::Message::Record(Message::FillWeight),
        ));
    content = content.push(crate::ui::card(converter));

    if let Some(error) = &state.error
    {
        content = content.push(crate::ui::flat_card(
            iced::widget::text(error.as_str())
                .size(13)
                .color(crate::style::DANGER),
        ));
    }

    content = content.push(crate::ui::primary_button_enabled(
        if state.saving
        {
            common.saving
        }
        else
        {
            text.save_record
        },
        if state.saving
        {
            None
        }
        else
        {
            Some(crate::app::Message::Record(Message::Save))
        },
    ));

    if state.last.is_some()
    {
        if state.confirm_delete
        {
            content = content.push(crate::ui::flat_card(
                iced::widget::column![
                    iced::widget::text(text.confirm_delete)
                        .size(14)
                        .color(crate::style::TEXT),
                    iced::widget::row![
                        crate::ui::danger_button(
                            common.yes,
                            crate::app::Message::Record(Message::Delete),
                        ),
                        crate::ui::secondary_button(
                            common.no,
                            crate::app::Message::Record(Message::DeleteCancel),
                        ),
                    ]
                    .spacing(10),
                ]
                .spacing(10),
            ));
        }
        else
        {
            content = content.push(crate::ui::danger_button(
                text.delete_record,
                crate::app::Message::Record(Message::DeleteConfirm),
            ));
        }
    }

    crate::ui::detail_page(
        app,
        state.exercise_name.as_str(),
        crate::app::Nav::Home(crate::app::Tab::Today),
        content.into(),
    )
}
