#[derive(Debug, Clone, Default)]
pub struct Item
{
    pub exercise_id: i64,
    pub name: String,
    pub sets: String,
    pub reps: String,
    pub weight: String,
    pub rest: String,
    pub key_points: String,
    pub note: String,
}

impl Item
{
    fn from_exercise(exercise: &crate::model::Exercise) -> Self
    {
        Self {
            exercise_id: exercise.id,
            name: exercise.name.clone(),
            sets: exercise.default_sets.to_string(),
            reps: exercise.default_reps.to_string(),
            weight: String::new(),
            rest: String::new(),
            key_points: exercise.key_points.clone(),
            note: String::new(),
        }
    }

    fn from_plan_item(
        item: &crate::model::PlanItem,
        exercise: Option<&crate::model::Exercise>,
    ) -> Self
    {
        let sets = item
            .plan_sets
            .or_else(|| exercise.map(|exercise| exercise.default_sets))
            .map(|value| value.to_string())
            .unwrap_or_default();
        let reps = item
            .plan_reps
            .or_else(|| exercise.map(|exercise| exercise.default_reps))
            .map(|value| value.to_string())
            .unwrap_or_default();
        let key_points = item
            .plan_key_points
            .clone()
            .or_else(|| exercise.map(|exercise| exercise.key_points.clone()))
            .unwrap_or_default();
        Self {
            exercise_id: item.exercise_id,
            name: item.exercise_name.clone(),
            sets,
            reps,
            weight: item
                .plan_weight
                .map(|value| format!("{value:.1}"))
                .unwrap_or_default(),
            rest: item
                .plan_rest
                .map(|value| value.to_string())
                .unwrap_or_default(),
            key_points,
            note: item.plan_note.clone().unwrap_or_default(),
        }
    }

    fn from_template_item(
        exercise: &crate::model::Exercise,
        template_item: Option<&crate::model::TemplateItem>,
    ) -> Self
    {
        let sets = template_item
            .and_then(|item| item.plan_sets)
            .unwrap_or(exercise.default_sets);
        let reps = template_item
            .and_then(|item| item.plan_reps)
            .unwrap_or(exercise.default_reps);
        Self {
            exercise_id: exercise.id,
            name: exercise.name.clone(),
            sets: sets.to_string(),
            reps: reps.to_string(),
            weight: String::new(),
            rest: String::new(),
            key_points: exercise.key_points.clone(),
            note: String::new(),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct State
{
    pub id: Option<i64>,
    pub phase_id: i64,
    pub phase_name: String,
    pub date: String,
    pub note: String,
    pub items: Vec<Item>,
    pub exercises: Vec<crate::model::Exercise>,
    pub saving: bool,
    pub error: Option<String>,
}

impl State
{
    pub fn new(
        phase_id: i64,
        phase_name: String,
        exercises: &[crate::model::Exercise],
        date: String,
    ) -> Self
    {
        Self {
            id: None,
            phase_id,
            phase_name,
            date,
            note: String::new(),
            items: Vec::new(),
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
        date: String,
    ) -> Self
    {
        let items = template
            .items
            .iter()
            .filter_map(|template_item| {
                exercises
                    .iter()
                    .find(|exercise| exercise.id == template_item.exercise_id)
                    .map(|exercise| Item::from_template_item(exercise, Some(template_item)))
            })
            .collect();
        Self {
            id: None,
            phase_id,
            phase_name,
            date,
            note: String::new(),
            items,
            exercises: exercises.to_vec(),
            saving: false,
            error: None,
        }
    }

    pub fn from_plan(
        phase_id: i64,
        phase_name: String,
        plan: &crate::model::Plan,
        exercises: &[crate::model::Exercise],
    ) -> Self
    {
        let items = plan
            .items
            .iter()
            .map(|item| {
                let exercise = exercises
                    .iter()
                    .find(|exercise| exercise.id == item.exercise_id);
                Item::from_plan_item(item, exercise)
            })
            .collect();
        Self {
            id: Some(plan.id),
            phase_id,
            phase_name,
            date: plan.date.clone(),
            note: plan.note.clone(),
            items,
            exercises: exercises.to_vec(),
            saving: false,
            error: None,
        }
    }
}

#[derive(Debug, Clone)]
pub enum Message
{
    Date(String),
    Note(String),
    Pick(String),
    ItemSets(usize, String),
    ItemReps(usize, String),
    ItemWeight(usize, String),
    ItemRest(usize, String),
    ItemKeyPoints(usize, String),
    ItemNote(usize, String),
    RemoveItem(usize),
    Save,
    Saved(Result<crate::model::Plan, crate::api::ApiError>),
}

pub fn update(app: &mut crate::app::App, message: Message) -> iced::Task<crate::app::Message>
{
    let text = crate::copy::plan::text(app.lang);
    match message
    {
        Message::Date(value) =>
        {
            app.plan_form.date = value;
            iced::Task::none()
        },
        Message::Note(value) =>
        {
            app.plan_form.note = value;
            iced::Task::none()
        },
        Message::Pick(label) =>
        {
            if let Some(exercise) = app
                .plan_form
                .exercises
                .iter()
                .find(|exercise| exercise.name == label)
            {
                if app
                    .plan_form
                    .items
                    .iter()
                    .any(|item| item.exercise_id == exercise.id)
                {
                    app.toast = Some(text.duplicate.to_string());
                }
                else
                {
                    app.plan_form.items.push(Item::from_exercise(exercise));
                }
            }
            iced::Task::none()
        },
        Message::ItemSets(index, value) =>
        {
            if let Some(item) = app.plan_form.items.get_mut(index)
            {
                item.sets = value;
            }
            iced::Task::none()
        },
        Message::ItemReps(index, value) =>
        {
            if let Some(item) = app.plan_form.items.get_mut(index)
            {
                item.reps = value;
            }
            iced::Task::none()
        },
        Message::ItemWeight(index, value) =>
        {
            if let Some(item) = app.plan_form.items.get_mut(index)
            {
                item.weight = value;
            }
            iced::Task::none()
        },
        Message::ItemRest(index, value) =>
        {
            if let Some(item) = app.plan_form.items.get_mut(index)
            {
                item.rest = value;
            }
            iced::Task::none()
        },
        Message::ItemKeyPoints(index, value) =>
        {
            if let Some(item) = app.plan_form.items.get_mut(index)
            {
                item.key_points = value;
            }
            iced::Task::none()
        },
        Message::ItemNote(index, value) =>
        {
            if let Some(item) = app.plan_form.items.get_mut(index)
            {
                item.note = value;
            }
            iced::Task::none()
        },
        Message::RemoveItem(index) =>
        {
            if index < app.plan_form.items.len()
            {
                app.plan_form.items.remove(index);
            }
            iced::Task::none()
        },
        Message::Save =>
        {
            if !crate::platform::valid_date(app.plan_form.date.trim())
            {
                app.plan_form.error = Some(text.invalid_date.to_string());
                return iced::Task::none();
            }
            if app.plan_form.items.is_empty()
            {
                app.plan_form.error = Some(text.no_items.to_string());
                return iced::Task::none();
            }
            let mut items = Vec::with_capacity(app.plan_form.items.len());
            for item in &app.plan_form.items
            {
                let sets = parse_optional::<i64>(&item.sets);
                let reps = parse_optional::<i64>(&item.reps);
                let weight = parse_optional::<f64>(&item.weight);
                let rest = parse_optional::<i64>(&item.rest);
                if sets.is_none() != item.sets.trim().is_empty()
                    || reps.is_none() != item.reps.trim().is_empty()
                    || weight.is_none() != item.weight.trim().is_empty()
                    || rest.is_none() != item.rest.trim().is_empty()
                {
                    app.plan_form.error = Some(text.invalid_number.to_string());
                    return iced::Task::none();
                }
                if sets.is_some_and(|value| value < 0)
                    || reps.is_some_and(|value| value < 0)
                    || weight.is_some_and(|value| value < 0.0)
                    || rest.is_some_and(|value| value < 0)
                {
                    app.plan_form.error = Some(text.invalid_number.to_string());
                    return iced::Task::none();
                }
                items.push(crate::model::PlanItemInput {
                    exercise_id: item.exercise_id,
                    plan_sets: sets,
                    plan_reps: reps,
                    plan_weight: weight,
                    plan_rest: rest,
                    plan_key_points: optional_text(&item.key_points),
                    plan_note: optional_text(&item.note),
                });
            }
            let input = crate::model::PlanInput {
                date: app.plan_form.date.trim().to_string(),
                note: app.plan_form.note.clone(),
                items,
            };
            let api = app.api.clone();
            let id = app.plan_form.id;
            let phase_id = app.plan_form.phase_id;
            let date_exists = text.date_exists_hint.to_string();
            app.plan_form.saving = true;
            app.plan_form.error = None;
            iced::Task::perform(
                async move {
                    match id
                    {
                        Some(id) => api.plan_update(id, &input).await,
                        None =>
                        {
                            let existing = api.plans(phase_id, Some(&input.date)).await?;
                            if !existing.is_empty()
                            {
                                return Err(crate::api::ApiError::Api(date_exists));
                            }
                            api.plan_create(phase_id, &input).await
                        },
                    }
                },
                |result| crate::app::Message::PlanForm(Message::Saved(result)),
            )
        },
        Message::Saved(result) => match result
        {
            Ok(_) =>
            {
                app.plan_form.saving = false;
                app.today.data = None;
                app.route = crate::app::Nav::Plans;
                app.toast = Some(crate::copy::common(app.lang).saved.to_string());
                crate::pages::plans::update(app, crate::pages::plans::Message::Refresh)
            },
            Err(error) =>
            {
                app.plan_form.saving = false;
                if matches!(error, crate::api::ApiError::Unauthorized)
                {
                    crate::app::report_error(app, error)
                }
                else
                {
                    app.plan_form.error = Some(error.to_string());
                    iced::Task::none()
                }
            },
        },
    }
}

fn parse_optional<T>(value: &str) -> Option<T>
where
    T: std::str::FromStr,
{
    let value = value.trim();
    if value.is_empty()
    {
        None
    }
    else
    {
        value.parse::<T>().ok()
    }
}

fn optional_text(value: &str) -> Option<String>
{
    let value = value.trim();
    if value.is_empty()
    {
        None
    }
    else
    {
        Some(value.to_string())
    }
}

pub fn view(app: &crate::app::App) -> crate::ui::El<'_>
{
    let text = crate::copy::plan::text(app.lang);
    let common = crate::copy::common(app.lang);
    let state = &app.plan_form;
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
            common.date,
            "2026-10-29",
            &state.date,
            |value| crate::app::Message::PlanForm(Message::Date(value)),
            false,
        ),
        crate::ui::text_field(
            text.plan_note,
            "",
            &state.note,
            |value| crate::app::Message::PlanForm(Message::Note(value)),
            false,
        ),
        crate::ui::pick_field(text.add_exercise, options, None, |label| {
            crate::app::Message::PlanForm(Message::Pick(label))
        },),
    ]
    .spacing(10);

    if state.items.is_empty()
    {
        content = content.push(crate::ui::muted(text.no_items));
    }
    for (index, item) in state.items.iter().enumerate()
    {
        let card = iced::widget::column![
            iced::widget::row![
                iced::widget::text(item.name.as_str())
                    .size(16)
                    .font(crate::style::bold())
                    .color(crate::style::TEXT),
                iced::widget::space().width(iced::Length::Fill),
                crate::ui::ghost_button(
                    common.remove,
                    crate::app::Message::PlanForm(Message::RemoveItem(index)),
                ),
            ]
            .align_y(iced::Alignment::Center),
            iced::widget::row![
                crate::ui::text_field(
                    common.sets,
                    "3",
                    &item.sets,
                    move |value| crate::app::Message::PlanForm(Message::ItemSets(index, value)),
                    false,
                )
                .width(iced::Length::Fill),
                crate::ui::text_field(
                    common.reps,
                    "8",
                    &item.reps,
                    move |value| crate::app::Message::PlanForm(Message::ItemReps(index, value)),
                    false,
                )
                .width(iced::Length::Fill),
            ]
            .spacing(8),
            iced::widget::row![
                crate::ui::text_field(
                    common.weight,
                    "",
                    &item.weight,
                    move |value| crate::app::Message::PlanForm(Message::ItemWeight(index, value)),
                    false,
                )
                .width(iced::Length::Fill),
                crate::ui::text_field(
                    common.rest,
                    "",
                    &item.rest,
                    move |value| crate::app::Message::PlanForm(Message::ItemRest(index, value)),
                    false,
                )
                .width(iced::Length::Fill),
            ]
            .spacing(8),
            crate::ui::text_field(
                common.key_points,
                "",
                &item.key_points,
                move |value| crate::app::Message::PlanForm(Message::ItemKeyPoints(index, value)),
                false,
            ),
            crate::ui::text_field(
                text.item_note,
                "",
                &item.note,
                move |value| crate::app::Message::PlanForm(Message::ItemNote(index, value)),
                false,
            ),
        ]
        .spacing(8);
        content = content.push(crate::ui::flat_card(card));
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
            Some(crate::app::Message::PlanForm(Message::Save))
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
        crate::app::Nav::Plans,
        content.into(),
    )
}
