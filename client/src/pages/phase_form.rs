#[derive(Debug, Clone, Default)]
pub struct State
{
    pub id: Option<i64>,
    pub name: String,
    pub note: String,
    pub start_date: String,
    pub saving: bool,
    pub error: Option<String>,
}

impl State
{
    pub fn new() -> Self
    {
        Self {
            id: None,
            name: String::new(),
            note: String::new(),
            start_date: crate::platform::today(),
            saving: false,
            error: None,
        }
    }

    pub fn from_phase(phase: &crate::model::Phase) -> Self
    {
        Self {
            id: Some(phase.id),
            name: phase.name.clone(),
            note: phase.note.clone(),
            start_date: phase.start_date.clone().unwrap_or_default(),
            saving: false,
            error: None,
        }
    }
}

#[derive(Debug, Clone)]
pub enum Message
{
    Name(String),
    Note(String),
    StartDate(String),
    Save,
    Saved(Result<crate::model::Phase, crate::api::ApiError>),
}

pub fn update(app: &mut crate::app::App, message: Message) -> iced::Task<crate::app::Message>
{
    let text = crate::copy::phase::text(app.lang);
    match message
    {
        Message::Name(value) =>
        {
            app.phase_form.name = value;
            iced::Task::none()
        },
        Message::Note(value) =>
        {
            app.phase_form.note = value;
            iced::Task::none()
        },
        Message::StartDate(value) =>
        {
            app.phase_form.start_date = value;
            iced::Task::none()
        },
        Message::Save =>
        {
            if app.phase_form.name.trim().is_empty()
            {
                app.phase_form.error = Some(crate::copy::common(app.lang).required.to_string());
                return iced::Task::none();
            }
            let start_date = app.phase_form.start_date.trim().to_string();
            if !start_date.is_empty() && !crate::platform::valid_date(&start_date)
            {
                app.phase_form.error = Some(text.start_hint.to_string());
                return iced::Task::none();
            }
            let input = crate::model::PhaseInput {
                name: app.phase_form.name.trim().to_string(),
                note: app.phase_form.note.clone(),
                start_date,
            };
            let api = app.api.clone();
            let id = app.phase_form.id;
            app.phase_form.saving = true;
            app.phase_form.error = None;
            iced::Task::perform(
                async move {
                    match id
                    {
                        Some(id) => api.phase_update(id, &input).await,
                        None => api.phase_create(&input).await,
                    }
                },
                |result| crate::app::Message::PhaseForm(Message::Saved(result)),
            )
        },
        Message::Saved(result) => match result
        {
            Ok(_) =>
            {
                app.phase_form.saving = false;
                app.today.data = None;
                app.route = crate::app::Nav::Phases;
                app.toast = Some(crate::copy::common(app.lang).saved.to_string());
                crate::pages::phases::update(app, crate::pages::phases::Message::Refresh)
            },
            Err(error) =>
            {
                app.phase_form.saving = false;
                if matches!(error, crate::api::ApiError::Unauthorized)
                {
                    crate::app::report_error(app, error)
                }
                else
                {
                    app.phase_form.error = Some(error.to_string());
                    iced::Task::none()
                }
            },
        },
    }
}

pub fn view(app: &crate::app::App) -> crate::ui::El<'_>
{
    let text = crate::copy::phase::text(app.lang);
    let common = crate::copy::common(app.lang);
    let state = &app.phase_form;
    let mut content = iced::widget::column![
        crate::ui::text_field(
            common.name,
            "增肌期",
            &state.name,
            |value| crate::app::Message::PhaseForm(Message::Name(value)),
            false,
        ),
        crate::ui::text_field(
            common.note,
            "",
            &state.note,
            |value| crate::app::Message::PhaseForm(Message::Note(value)),
            false,
        ),
        crate::ui::text_field(
            text.start_date,
            text.start_hint,
            &state.start_date,
            |value| crate::app::Message::PhaseForm(Message::StartDate(value)),
            false,
        ),
    ]
    .spacing(12);
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
            Some(crate::app::Message::PhaseForm(Message::Save))
        },
    ));
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
        crate::app::Nav::Phases,
        content.into(),
    )
}
