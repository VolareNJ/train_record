#[derive(Debug, Clone, Default)]
pub struct State
{
    pub id: Option<i64>,
    pub phase_id: i64,
    pub phase_name: String,
    pub name: String,
    pub selected: Vec<i64>,
    pub exercises: Vec<crate::model::Exercise>,
    pub saving: bool,
    pub error: Option<String>,
}

impl State
{
    pub fn new(phase_id: i64, phase_name: String, exercises: &[crate::model::Exercise]) -> Self
    {
        Self {
            id: None,
            phase_id,
            phase_name,
            name: String::new(),
            selected: Vec::new(),
            exercises: exercises.to_vec(),
            saving: false,
            error: None,
        }
    }

    pub fn from_template(
        phase_id: i64,
        phase_name: String,
        template: &crate::model::Template,
        exercises: &[crate::model::Exercise],
    ) -> Self
    {
        Self {
            id: Some(template.id),
            phase_id,
            phase_name,
            name: template.name.clone(),
            selected: template.items.iter().map(|item| item.exercise_id).collect(),
            exercises: exercises.to_vec(),
            saving: false,
            error: None,
        }
    }
}

#[derive(Debug, Clone)]
pub enum Message
{
    Name(String),
    Pick(String),
    MoveUp(usize),
    MoveDown(usize),
    Remove(usize),
    Save,
    Saved(Result<crate::model::Template, crate::api::ApiError>),
}

pub fn update(app: &mut crate::app::App, message: Message) -> iced::Task<crate::app::Message>
{
    match message
    {
        Message::Name(value) =>
        {
            app.template_form.name = value;
            iced::Task::none()
        },
        Message::Pick(label) =>
        {
            if let Some(exercise) = app
                .template_form
                .exercises
                .iter()
                .find(|exercise| exercise.name == label)
                && !app.template_form.selected.contains(&exercise.id)
            {
                app.template_form.selected.push(exercise.id);
            }
            iced::Task::none()
        },
        Message::MoveUp(index) =>
        {
            if index > 0 && index < app.template_form.selected.len()
            {
                app.template_form.selected.swap(index, index - 1);
            }
            iced::Task::none()
        },
        Message::MoveDown(index) =>
        {
            if index + 1 < app.template_form.selected.len()
            {
                app.template_form.selected.swap(index, index + 1);
            }
            iced::Task::none()
        },
        Message::Remove(index) =>
        {
            if index < app.template_form.selected.len()
            {
                app.template_form.selected.remove(index);
            }
            iced::Task::none()
        },
        Message::Save =>
        {
            let text = crate::copy::template::text(app.lang);
            if app.template_form.name.trim().is_empty()
            {
                app.template_form.error = Some(crate::copy::common(app.lang).required.to_string());
                return iced::Task::none();
            }
            if app.template_form.selected.is_empty()
            {
                app.template_form.error = Some(text.no_items.to_string());
                return iced::Task::none();
            }
            let input = crate::model::TemplateInput {
                name: app.template_form.name.trim().to_string(),
                items: app
                    .template_form
                    .selected
                    .iter()
                    .map(|exercise_id| crate::model::TemplateItemInput {
                        exercise_id: *exercise_id,
                    })
                    .collect(),
            };
            let api = app.api.clone();
            let id = app.template_form.id;
            let phase_id = app.template_form.phase_id;
            app.template_form.saving = true;
            app.template_form.error = None;
            iced::Task::perform(
                async move {
                    match id
                    {
                        Some(id) => api.template_update(id, &input).await,
                        None => api.template_create(phase_id, &input).await,
                    }
                },
                |result| crate::app::Message::TemplateForm(Message::Saved(result)),
            )
        },
        Message::Saved(result) => match result
        {
            Ok(_) =>
            {
                app.template_form.saving = false;
                app.route = crate::app::Nav::Templates;
                app.toast = Some(crate::copy::common(app.lang).saved.to_string());
                crate::pages::templates::update(app, crate::pages::templates::Message::Refresh)
            },
            Err(error) =>
            {
                app.template_form.saving = false;
                if matches!(error, crate::api::ApiError::Unauthorized)
                {
                    crate::app::report_error(app, error)
                }
                else
                {
                    app.template_form.error = Some(error.to_string());
                    iced::Task::none()
                }
            },
        },
    }
}

pub fn view(app: &crate::app::App) -> crate::ui::El<'_>
{
    let text = crate::copy::template::text(app.lang);
    let common = crate::copy::common(app.lang);
    let state = &app.template_form;
    let options: Vec<String> = state
        .exercises
        .iter()
        .map(|exercise| exercise.name.clone())
        .collect();
    let mut content = iced::widget::column![
        iced::widget::text(state.phase_name.as_str())
            .size(13)
            .color(crate::style::MUTED),
        crate::ui::text_field(
            common.name,
            "A 分化",
            &state.name,
            |value| crate::app::Message::TemplateForm(Message::Name(value)),
            false,
        ),
        crate::ui::pick_field(text.pick_exercise, options, None, |label| {
            crate::app::Message::TemplateForm(Message::Pick(label))
        },),
        crate::ui::section_title(text.selected),
    ]
    .spacing(10);

    if state.selected.is_empty()
    {
        content = content.push(crate::ui::muted(text.no_items));
    }
    for (index, exercise_id) in state.selected.iter().enumerate()
    {
        let name = state
            .exercises
            .iter()
            .find(|exercise| exercise.id == *exercise_id)
            .map(|exercise| exercise.name.as_str())
            .unwrap_or("?");
        content = content.push(
            iced::widget::row![
                iced::widget::text(name)
                    .size(15)
                    .color(crate::style::TEXT)
                    .width(iced::Length::Fill),
                crate::ui::ghost_button(
                    text.move_up,
                    crate::app::Message::TemplateForm(Message::MoveUp(index)),
                ),
                crate::ui::ghost_button(
                    text.move_down,
                    crate::app::Message::TemplateForm(Message::MoveDown(index)),
                ),
                crate::ui::ghost_button(
                    common.remove,
                    crate::app::Message::TemplateForm(Message::Remove(index)),
                ),
            ]
            .spacing(4)
            .align_y(iced::Alignment::Center),
        );
    }

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
            Some(crate::app::Message::TemplateForm(Message::Save))
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
        crate::app::Nav::Templates,
        content.into(),
    )
}
