#[derive(Debug, Clone, Default)]
pub struct State
{
    pub id: Option<i64>,
    pub name: String,
    pub body_part: String,
    pub mode: String,
    pub bar_weight: String,
    pub unit: String,
    pub sets: String,
    pub reps: String,
    pub key_points: String,
    pub saving: bool,
    pub error: Option<String>,
    pub confirm_delete: bool,
}

impl State
{
    pub fn new() -> Self
    {
        Self {
            id: None,
            name: String::new(),
            body_part: String::new(),
            mode: "bar".to_string(),
            bar_weight: "20".to_string(),
            unit: "kg".to_string(),
            sets: "3".to_string(),
            reps: "8".to_string(),
            key_points: String::new(),
            saving: false,
            error: None,
            confirm_delete: false,
        }
    }

    pub fn from_exercise(exercise: &crate::model::Exercise) -> Self
    {
        Self {
            id: Some(exercise.id),
            name: exercise.name.clone(),
            body_part: exercise.body_part.clone(),
            mode: exercise.default_mode.clone(),
            bar_weight: format!("{:.1}", exercise.bar_weight),
            unit: exercise.default_unit.clone(),
            sets: exercise.default_sets.to_string(),
            reps: exercise.default_reps.to_string(),
            key_points: exercise.key_points.clone(),
            saving: false,
            error: None,
            confirm_delete: false,
        }
    }
}

#[derive(Debug, Clone)]
pub enum Message
{
    Name(String),
    BodyPart(String),
    Mode(String),
    BarWeight(String),
    Unit(String),
    Sets(String),
    Reps(String),
    KeyPoints(String),
    Save,
    Saved(Result<crate::model::Exercise, crate::api::ApiError>),
    Delete,
    DeleteConfirmed(i64),
    DeleteCancel,
    Deleted(Result<serde_json::Value, crate::api::ApiError>),
}

pub fn update(app: &mut crate::app::App, message: Message) -> iced::Task<crate::app::Message>
{
    let text = crate::copy::library::text(app.lang);
    match message
    {
        Message::Name(value) =>
        {
            app.exercise_form.name = value;
            iced::Task::none()
        },
        Message::BodyPart(value) =>
        {
            app.exercise_form.body_part = value;
            iced::Task::none()
        },
        Message::Mode(value) =>
        {
            app.exercise_form.mode = value;
            iced::Task::none()
        },
        Message::BarWeight(value) =>
        {
            app.exercise_form.bar_weight = value;
            iced::Task::none()
        },
        Message::Unit(value) =>
        {
            app.exercise_form.unit = value;
            iced::Task::none()
        },
        Message::Sets(value) =>
        {
            app.exercise_form.sets = value;
            iced::Task::none()
        },
        Message::Reps(value) =>
        {
            app.exercise_form.reps = value;
            iced::Task::none()
        },
        Message::KeyPoints(value) =>
        {
            app.exercise_form.key_points = value;
            iced::Task::none()
        },
        Message::Save =>
        {
            if app.exercise_form.name.trim().is_empty()
                || app.exercise_form.body_part.trim().is_empty()
            {
                app.exercise_form.error = Some(crate::copy::common(app.lang).required.to_string());
                return iced::Task::none();
            }
            let bar_weight = app.exercise_form.bar_weight.trim().parse::<f64>();
            let sets = app.exercise_form.sets.trim().parse::<i64>();
            let reps = app.exercise_form.reps.trim().parse::<i64>();
            let (Ok(bar_weight), Ok(sets), Ok(reps)) = (bar_weight, sets, reps)
            else
            {
                app.exercise_form.error = Some(text.tip.to_string());
                return iced::Task::none();
            };
            if bar_weight < 0.0 || sets < 0 || reps < 0
            {
                app.exercise_form.error = Some(text.tip.to_string());
                return iced::Task::none();
            }
            let input = crate::model::ExerciseInput {
                name: app.exercise_form.name.trim().to_string(),
                body_part: app.exercise_form.body_part.trim().to_string(),
                default_mode: app.exercise_form.mode.clone(),
                bar_weight,
                default_unit: app.exercise_form.unit.clone(),
                default_sets: sets,
                default_reps: reps,
                key_points: app.exercise_form.key_points.clone(),
            };
            let api = app.api.clone();
            let id = app.exercise_form.id;
            app.exercise_form.saving = true;
            app.exercise_form.error = None;
            iced::Task::perform(
                async move {
                    match id
                    {
                        Some(id) => api.exercise_update(id, &input).await,
                        None => api.exercise_create(&input).await,
                    }
                },
                |result| crate::app::Message::ExerciseForm(Message::Saved(result)),
            )
        },
        Message::Saved(result) => match result
        {
            Ok(_) =>
            {
                app.exercise_form.saving = false;
                app.today.data = None;
                app.route = crate::app::Nav::Home(crate::app::Tab::Library);
                app.tab = crate::app::Tab::Library;
                app.toast = Some(crate::copy::common(app.lang).saved.to_string());
                crate::pages::library::update(app, crate::pages::library::Message::Refresh)
            },
            Err(error) =>
            {
                app.exercise_form.saving = false;
                if matches!(error, crate::api::ApiError::Unauthorized)
                {
                    crate::app::report_error(app, error)
                }
                else
                {
                    app.exercise_form.error = Some(error.to_string());
                    iced::Task::none()
                }
            },
        },
        Message::Delete =>
        {
            app.exercise_form.confirm_delete = true;
            iced::Task::none()
        },
        Message::DeleteConfirmed(id) =>
        {
            let api = app.api.clone();
            iced::Task::perform(async move { api.exercise_delete(id).await }, |result| {
                crate::app::Message::ExerciseForm(Message::Deleted(result))
            })
        },
        Message::DeleteCancel =>
        {
            app.exercise_form.confirm_delete = false;
            iced::Task::none()
        },
        Message::Deleted(result) => match result
        {
            Ok(_) =>
            {
                app.exercise_form.confirm_delete = false;
                app.route = crate::app::Nav::Home(crate::app::Tab::Library);
                app.tab = crate::app::Tab::Library;
                app.toast = Some(crate::copy::common(app.lang).saved.to_string());
                crate::pages::library::update(app, crate::pages::library::Message::Refresh)
            },
            Err(error) => crate::app::report_error(app, error),
        },
    }
}

fn mode_options(text: &crate::copy::library::Text) -> Vec<String>
{
    vec![
        text.mode_bar.to_string(),
        text.mode_std.to_string(),
        text.mode_support.to_string(),
    ]
}

fn mode_label(text: &crate::copy::library::Text, mode: &str) -> String
{
    match mode
    {
        "std" => text.mode_std.to_string(),
        "support" => text.mode_support.to_string(),
        _ => text.mode_bar.to_string(),
    }
}

fn mode_code(text: &crate::copy::library::Text, label: &str) -> String
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
    let text = crate::copy::library::text(app.lang);
    let common = crate::copy::common(app.lang);
    let state = &app.exercise_form;
    let mode = mode_label(&text, &state.mode);
    let unit = if state.unit == "lb"
    {
        text.unit_lb.to_string()
    }
    else
    {
        text.unit_kg.to_string()
    };
    let mut content = iced::widget::column![
        crate::ui::section_title(text.basic),
        crate::ui::text_field(
            common.name,
            "深蹲",
            &state.name,
            |value| crate::app::Message::ExerciseForm(Message::Name(value)),
            false,
        ),
        crate::ui::text_field(
            text.body_part,
            "胸 / 背 / 腿",
            &state.body_part,
            |value| crate::app::Message::ExerciseForm(Message::BodyPart(value)),
            false,
        ),
        crate::ui::section_title(text.defaults),
        crate::ui::pick_field(
            text.default_mode,
            mode_options(&text),
            Some(mode),
            move |label| crate::app::Message::ExerciseForm(Message::Mode(mode_code(&text, &label))),
        ),
        crate::ui::text_field(
            text.bar_weight,
            "20",
            &state.bar_weight,
            |value| crate::app::Message::ExerciseForm(Message::BarWeight(value)),
            false,
        ),
        crate::ui::pick_field(
            text.default_unit,
            vec![text.unit_kg.to_string(), text.unit_lb.to_string()],
            Some(unit),
            move |label| {
                crate::app::Message::ExerciseForm(Message::Unit(
                    if label == text.unit_lb { "lb" } else { "kg" }.to_string(),
                ))
            },
        ),
        iced::widget::row![
            crate::ui::text_field(
                text.default_sets,
                "3",
                &state.sets,
                |value| crate::app::Message::ExerciseForm(Message::Sets(value)),
                false,
            )
            .width(iced::Length::Fill),
            crate::ui::text_field(
                text.default_reps,
                "8",
                &state.reps,
                |value| crate::app::Message::ExerciseForm(Message::Reps(value)),
                false,
            )
            .width(iced::Length::Fill),
        ]
        .spacing(10),
        crate::ui::text_field(
            text.key_points,
            text.tip,
            &state.key_points,
            |value| crate::app::Message::ExerciseForm(Message::KeyPoints(value)),
            false,
        ),
    ]
    .spacing(10);

    if let Some(error) = &state.error
    {
        content = content.push(
            iced::widget::text(error.as_str())
                .size(13)
                .color(crate::style::DANGER),
        );
    }
    content = content.push(crate::ui::primary_button_enabled(
        if state.saving
        {
            common.saving
        }
        else
        {
            common.save
        },
        if state.saving
        {
            None
        }
        else
        {
            Some(crate::app::Message::ExerciseForm(Message::Save))
        },
    ));
    if state.id.is_some()
    {
        if state.confirm_delete
        {
            content = content.push(
                iced::widget::row![
                    iced::widget::text(text.delete_confirm)
                        .size(12)
                        .color(crate::style::DANGER),
                    crate::ui::danger_button(
                        common.yes,
                        crate::app::Message::ExerciseForm(Message::DeleteConfirmed(
                            state.id.unwrap_or(0),
                        )),
                    ),
                    crate::ui::secondary_button(
                        common.no,
                        crate::app::Message::ExerciseForm(Message::DeleteCancel),
                    ),
                ]
                .spacing(6),
            );
        }
        else
        {
            content = content.push(crate::ui::danger_button(
                common.delete,
                crate::app::Message::ExerciseForm(Message::Delete),
            ));
        }
    }

    crate::ui::detail_page(
        app,
        if state.id.is_some()
        {
            text.edit_title
        }
        else
        {
            text.create_title
        },
        crate::app::Nav::Home(crate::app::Tab::Library),
        content.into(),
    )
}
