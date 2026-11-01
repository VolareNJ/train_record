#[derive(Debug, Clone, Default)]
pub struct State
{
    pub plan_id: i64,
    pub item_id: i64,
    pub exercise_id: i64,
    pub exercise_name: String,
    pub body_part: String,
    pub plan_sets: Option<i64>,
    pub plan_reps: Option<i64>,
    pub plan_weight: Option<f64>,
    pub plan_rest: Option<i64>,
    pub plan_key_points: Option<String>,
    pub plan_note: String,
    pub item_note: Option<String>,
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
    pub unit: String,
    pub converter_value: String,
    pub bar_weight: String,
    pub body_weight: String,
    pub sync_defaults: bool,
    pub chart: Option<Vec<crate::model::ExerciseRecord>>,
    pub saving: bool,
    pub error: Option<String>,
    pub confirm_delete: bool,
}

impl State
{
    pub fn from_item(app: &crate::app::App, plan_id: i64, item: &crate::model::TodayItem) -> Self
    {
        let today = crate::platform::today();
        let today_record = item
            .last_record
            .as_ref()
            .filter(|record| record.record_date == today);
        let last = item.last_record.clone();
        let weight = today_record
            .map(|record| format!("{:.1}", record.weight))
            .or_else(|| item.plan_weight.map(|weight| format!("{weight:.1}")))
            .or_else(|| last.as_ref().map(|record| format!("{:.1}", record.weight)))
            .unwrap_or_default();
        let sets = today_record
            .map(|record| record.sets)
            .or(item.plan_sets)
            .or_else(|| last.as_ref().map(|record| record.sets))
            .unwrap_or(item.default_sets);
        let reps = today_record
            .map(|record| record.reps)
            .or(item.plan_reps)
            .or_else(|| last.as_ref().map(|record| record.reps))
            .unwrap_or(item.default_reps);
        let rest = today_record
            .map(|record| record.rest)
            .or(item.plan_rest)
            .or_else(|| last.as_ref().map(|record| record.rest))
            .unwrap_or(0);
        let mode = today_record
            .map(|record| record.mode.clone())
            .or_else(|| last.as_ref().map(|record| record.mode.clone()))
            .unwrap_or_else(|| item.default_mode.clone());
        let key_points = today_record
            .map(|record| record.key_points.clone())
            .or(item.plan_key_points.clone())
            .or_else(|| last.as_ref().map(|record| record.key_points.clone()))
            .unwrap_or_else(|| item.default_key_points.clone());
        let body_weight = app
            .user
            .as_ref()
            .and_then(|user| user.body_weight)
            .map(|weight| format!("{weight:.1}"))
            .unwrap_or_else(|| "70".to_string());

        Self {
            plan_id,
            item_id: item.id,
            exercise_id: item.exercise_id,
            exercise_name: item.exercise_name.clone(),
            body_part: item.body_part.clone(),
            plan_sets: item.plan_sets,
            plan_reps: item.plan_reps,
            plan_weight: item.plan_weight,
            plan_rest: item.plan_rest,
            plan_key_points: item.plan_key_points.clone(),
            plan_note: String::new(),
            item_note: item.plan_note.clone(),
            last,
            weight,
            sets: sets.to_string(),
            reps: reps.to_string(),
            rest: rest.to_string(),
            feeling: today_record
                .map(|record| record.feeling.clone())
                .unwrap_or_default(),
            strategy: today_record
                .map(|record| record.strategy.clone())
                .unwrap_or_default(),
            key_points,
            completed: today_record.is_some_and(|record| record.completed),
            mode,
            unit: item.default_unit.clone(),
            converter_value: String::new(),
            bar_weight: format!("{}", item.bar_weight),
            body_weight,
            sync_defaults: false,
            chart: None,
            saving: false,
            error: None,
            confirm_delete: false,
        }
    }

    fn total(&self, fallback_body_weight: Option<f64>) -> Option<f64>
    {
        let raw = self.converter_value.trim().parse::<f64>().ok()?;
        let raw_kg = if self.unit == "lb" { raw * 0.4536 } else { raw };
        match self.mode.as_str()
        {
            "bar" => self
                .bar_weight
                .trim()
                .parse::<f64>()
                .ok()
                .map(|bar| bar + raw_kg * 2.0),
            "std" => Some(raw_kg),
            "support" =>
            {
                let body = self
                    .body_weight
                    .trim()
                    .parse::<f64>()
                    .ok()
                    .or(fallback_body_weight)
                    .unwrap_or(70.0);
                Some((body - raw_kg).max(0.0))
            },
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
    Unit(String),
    ConverterValue(String),
    BarWeight(String),
    BodyWeight(String),
    SyncDefaults(bool),
    FillWeight,
    Save,
    Saved(Result<serde_json::Value, crate::api::ApiError>),
    Delete,
    DeleteConfirm,
    DeleteCancel,
    Deleted(Result<serde_json::Value, crate::api::ApiError>),
    ChartLoaded(Result<crate::model::ExerciseStats, crate::api::ApiError>),
    ConfigSaved(Result<serde_json::Value, crate::api::ApiError>),
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
            save_config(app)
        },
        Message::Unit(value) =>
        {
            app.record.unit = value;
            save_config(app)
        },
        Message::ConverterValue(value) =>
        {
            app.record.converter_value = value;
            iced::Task::none()
        },
        Message::BarWeight(value) =>
        {
            app.record.bar_weight = value;
            save_config(app)
        },
        Message::BodyWeight(value) =>
        {
            app.record.body_weight = value;
            iced::Task::none()
        },
        Message::SyncDefaults(value) =>
        {
            app.record.sync_defaults = value;
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
            let bar_weight = app.record.bar_weight.trim().parse::<f64>().unwrap_or(0.0);
            let input = crate::model::RecordInput {
                weight,
                sets,
                reps,
                rest,
                feeling: app.record.feeling.clone(),
                strategy: app.record.strategy.clone(),
                key_points: app.record.key_points.clone(),
                completed: app.record.completed,
                mode: app.record.mode.clone(),
                bar_weight,
                unit: app.record.unit.clone(),
                sync_defaults: app.record.sync_defaults,
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
        Message::ChartLoaded(result) => match result
        {
            Ok(stats) =>
            {
                app.record.chart = Some(stats.records);
                iced::Task::none()
            },
            Err(_) => iced::Task::none(),
        },
        Message::ConfigSaved(result) => match result
        {
            Ok(_) => iced::Task::none(),
            Err(error) => match error
            {
                crate::api::ApiError::Unauthorized => crate::app::report_error(app, error),
                other =>
                {
                    app.record.error = Some(other.to_string());
                    iced::Task::none()
                },
            },
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

fn save_config(app: &mut crate::app::App) -> iced::Task<crate::app::Message>
{
    let api = app.api.clone();
    let id = app.record.exercise_id;
    let mode = app.record.mode.clone();
    let bar_weight = app.record.bar_weight.trim().parse::<f64>().unwrap_or(0.0);
    let unit = app.record.unit.clone();
    iced::Task::perform(
        async move { api.exercise_config(id, &mode, bar_weight, &unit).await },
        |result| crate::app::Message::Record(Message::ConfigSaved(result)),
    )
}

fn mode_options(text: &crate::copy::record::Text) -> Vec<String>
{
    vec![
        text.mode_bar.to_string(),
        text.mode_std.to_string(),
        text.mode_support.to_string(),
    ]
}

fn mode_label(text: &crate::copy::record::Text, mode: &str) -> String
{
    match mode
    {
        "std" => text.mode_std.to_string(),
        "support" => text.mode_support.to_string(),
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

    if !state.plan_note.trim().is_empty()
        || state
            .item_note
            .as_deref()
            .is_some_and(|note| !note.trim().is_empty())
    {
        content = content.push(crate::ui::flat_card(
            iced::widget::column![
                iced::widget::text(text.plan_note)
                    .size(12)
                    .color(crate::style::MUTED),
                iced::widget::text(state.plan_note.as_str())
                    .size(13)
                    .color(crate::style::TEXT),
                iced::widget::text(text.item_note)
                    .size(12)
                    .color(crate::style::MUTED),
                iced::widget::text(state.item_note.as_deref().unwrap_or(common.not_set))
                    .size(13)
                    .color(crate::style::TEXT),
            ]
            .spacing(4),
        ));
    }

    if let Some(chart) = &state.chart
        && !chart.is_empty()
    {
        content = content.push(crate::ui::card(
            iced::widget::column![
                crate::ui::section_title(text.trend),
                crate::pages::stats::chart(chart),
            ]
            .spacing(8),
        ));
    }

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
        crate::ui::checkbox_field(text.sync_defaults, state.sync_defaults, |value| {
            crate::app::Message::Record(Message::SyncDefaults(value))
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
        crate::ui::pick_field(
            text.unit,
            vec!["kg".to_string(), "lb".to_string()],
            Some(state.unit.clone()),
            |value| crate::app::Message::Record(Message::Unit(value)),
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
        converter = converter.push(crate::ui::pick_field(
            text.bar_weight,
            vec![
                "20".to_string(),
                "11.3".to_string(),
                "10".to_string(),
                "0".to_string(),
            ],
            Some(state.bar_weight.clone()),
            |value| crate::app::Message::Record(Message::BarWeight(value)),
        ));
    }
    if state.mode == "support"
    {
        converter = converter.push(crate::ui::text_field(
            text.body_weight,
            "70",
            &state.body_weight,
            |value| crate::app::Message::Record(Message::BodyWeight(value)),
            false,
        ));
    }
    converter = converter
        .push(crate::ui::centered(
            iced::widget::text(format!("{}: {}", text.total, total_text))
                .size(18)
                .font(crate::style::bold())
                .color(crate::style::PRIMARY_LIGHT),
        ))
        .push(crate::ui::centered(crate::ui::secondary_button(
            text.fill_weight,
            crate::app::Message::Record(Message::FillWeight),
        )));
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
    content = content.push(crate::ui::centered(crate::ui::secondary_button(
        text.trend,
        crate::app::Message::OpenStats(state.exercise_id, state.exercise_name.clone()),
    )));

    if state.last.is_some()
    {
        if state.confirm_delete
        {
            content = content.push(crate::ui::flat_card(
                iced::widget::column![
                    iced::widget::text(text.confirm_delete)
                        .size(14)
                        .color(crate::style::TEXT),
                    crate::ui::action_row([
                        crate::ui::danger_button(
                            common.yes,
                            crate::app::Message::Record(Message::Delete),
                        ),
                        crate::ui::secondary_button(
                            common.no,
                            crate::app::Message::Record(Message::DeleteCancel),
                        ),
                    ]),
                ]
                .spacing(10),
            ));
        }
        else
        {
            content = content.push(crate::ui::centered(crate::ui::danger_outline_button(
                text.delete_record,
                crate::app::Message::Record(Message::DeleteConfirm),
            )));
        }
    }

    crate::ui::detail_page(
        app,
        state.exercise_name.as_str(),
        crate::app::Nav::Home(crate::app::Tab::Today),
        content.into(),
    )
}
