#[derive(Debug, Clone, Default)]
pub struct State
{
    pub items: Vec<crate::model::Phase>,
    pub loading: bool,
    pub error: Option<String>,
    pub confirm_archive: Option<i64>,
}

#[derive(Debug, Clone)]
pub enum Message
{
    Refresh,
    Loaded(Result<Vec<crate::model::Phase>, crate::api::ApiError>),
    New,
    Edit(i64),
    Archive(i64, bool),
    ArchiveConfirmed(i64),
    Archived(Result<crate::model::Phase, crate::api::ApiError>),
    CancelArchive,
    Templates(i64, String),
    Plans(i64, String),
}

pub fn update(app: &mut crate::app::App, message: Message) -> iced::Task<crate::app::Message>
{
    match message
    {
        Message::Refresh =>
        {
            app.phases.loading = true;
            app.phases.error = None;
            let api = app.api.clone();
            iced::Task::perform(async move { api.phases().await }, |result| {
                crate::app::Message::Phases(Message::Loaded(result))
            })
        },
        Message::Loaded(result) => match result
        {
            Ok(items) =>
            {
                app.phases.loading = false;
                app.phases.error = None;
                app.phases.items = items;
                iced::Task::none()
            },
            Err(error) =>
            {
                app.phases.loading = false;
                if matches!(error, crate::api::ApiError::Unauthorized)
                {
                    crate::app::report_error(app, error)
                }
                else
                {
                    app.phases.error = Some(error.to_string());
                    iced::Task::none()
                }
            },
        },
        Message::New =>
        {
            app.phase_form = crate::pages::phase_form::State::new();
            app.route = crate::app::Nav::PhaseForm;
            iced::Task::none()
        },
        Message::Edit(id) =>
        {
            if let Some(phase) = app.phases.items.iter().find(|phase| phase.id == id)
            {
                app.phase_form = crate::pages::phase_form::State::from_phase(phase);
                app.route = crate::app::Nav::PhaseForm;
            }
            iced::Task::none()
        },
        Message::Archive(id, archived) =>
        {
            if archived
            {
                app.phases.confirm_archive = Some(id);
                iced::Task::none()
            }
            else
            {
                archive(app, id, false)
            }
        },
        Message::ArchiveConfirmed(id) =>
        {
            let api = app.api.clone();
            iced::Task::perform(async move { api.phase_archive(id, true).await }, |result| {
                crate::app::Message::Phases(Message::Archived(result))
            })
        },
        Message::Archived(result) => match result
        {
            Ok(_) =>
            {
                app.phases.confirm_archive = None;
                app.today.data = None;
                app.toast = Some(crate::copy::common(app.lang).saved.to_string());
                update(app, Message::Refresh)
            },
            Err(error) => crate::app::report_error(app, error),
        },
        Message::CancelArchive =>
        {
            app.phases.confirm_archive = None;
            iced::Task::none()
        },
        Message::Templates(id, name) =>
        {
            app.templates.phase_id = id;
            app.templates.phase_name = name;
            app.templates.items.clear();
            app.route = crate::app::Nav::Templates;
            crate::pages::templates::update(app, crate::pages::templates::Message::Refresh)
        },
        Message::Plans(id, name) =>
        {
            app.plans.phase_id = id;
            app.plans.phase_name = name;
            app.plans.plans.clear();
            app.plans.templates.clear();
            app.route = crate::app::Nav::Plans;
            crate::pages::plans::update(app, crate::pages::plans::Message::Refresh)
        },
    }
}

fn archive(app: &mut crate::app::App, id: i64, archived: bool) -> iced::Task<crate::app::Message>
{
    let api = app.api.clone();
    iced::Task::perform(
        async move { api.phase_archive(id, archived).await },
        |result| crate::app::Message::Phases(Message::Archived(result)),
    )
}

pub fn view(app: &crate::app::App) -> crate::ui::El<'_>
{
    let text = crate::copy::phase::text(app.lang);
    let common = crate::copy::common(app.lang);
    let mut content = iced::widget::column![].spacing(12);
    content = content.push(crate::ui::primary_button(
        text.new_phase,
        crate::app::Message::Phases(Message::New),
    ));
    if app.phases.loading
    {
        content = content.push(crate::ui::loading(common.loading));
    }
    if let Some(error) = &app.phases.error
    {
        content = content.push(crate::ui::error_card(
            app.lang,
            error,
            Some(crate::ui::secondary_button(
                common.retry,
                crate::app::Message::Phases(Message::Refresh),
            )),
        ));
    }
    if app.phases.items.is_empty() && !app.phases.loading
    {
        content = content.push(crate::ui::empty_card(
            text.empty,
            text.archived_hint,
            Some(crate::ui::secondary_button(
                text.new_phase,
                crate::app::Message::Phases(Message::New),
            )),
        ));
    }

    let active: Vec<&crate::model::Phase> = app
        .phases
        .items
        .iter()
        .filter(|phase| !phase.archived)
        .collect();
    let archived: Vec<&crate::model::Phase> = app
        .phases
        .items
        .iter()
        .filter(|phase| phase.archived)
        .collect();

    for (title, phases) in [(text.active, active), (text.archived, archived)]
    {
        if phases.is_empty()
        {
            continue;
        }
        content = content.push(crate::ui::section_title(title));
        for phase in phases
        {
            let mut card = iced::widget::column![
                iced::widget::row![
                    iced::widget::text(phase.name.as_str())
                        .size(18)
                        .font(crate::style::bold())
                        .color(crate::style::TEXT),
                    iced::widget::space().width(iced::Length::Fill),
                    crate::ui::badge(
                        if phase.archived
                        {
                            text.archived
                        }
                        else
                        {
                            text.active
                        },
                        if phase.archived
                        {
                            crate::style::MUTED
                        }
                        else
                        {
                            crate::style::SUCCESS
                        },
                    ),
                ]
                .align_y(iced::Alignment::Center),
                iced::widget::text(common.days_text(phase.days))
                    .size(13)
                    .color(crate::style::MUTED),
                iced::widget::text(common.or_not_set(&phase.note))
                    .size(13)
                    .color(crate::style::TEXT),
                iced::widget::row![
                    crate::ui::secondary_button(
                        text.templates,
                        crate::app::Message::Phases(Message::Templates(
                            phase.id,
                            phase.name.clone(),
                        )),
                    ),
                    crate::ui::secondary_button(
                        text.plans,
                        crate::app::Message::Phases(Message::Plans(phase.id, phase.name.clone(),)),
                    ),
                    crate::ui::secondary_button(
                        common.edit,
                        crate::app::Message::Phases(Message::Edit(phase.id)),
                    ),
                ]
                .spacing(8),
            ]
            .spacing(8);

            if app.phases.confirm_archive == Some(phase.id)
            {
                card = card.push(
                    iced::widget::column![
                        iced::widget::text(text.archive_confirm)
                            .size(12)
                            .color(crate::style::DANGER),
                        iced::widget::row![
                            crate::ui::danger_button(
                                common.yes,
                                crate::app::Message::Phases(Message::ArchiveConfirmed(phase.id)),
                            ),
                            crate::ui::secondary_button(
                                common.no,
                                crate::app::Message::Phases(Message::CancelArchive),
                            ),
                        ]
                        .spacing(8),
                    ]
                    .spacing(6),
                );
            }
            else
            {
                card = card.push(crate::ui::ghost_button(
                    if phase.archived
                    {
                        text.unarchive
                    }
                    else
                    {
                        text.archive
                    },
                    crate::app::Message::Phases(Message::Archive(phase.id, !phase.archived)),
                ));
            }
            content = content.push(crate::ui::card(card));
        }
    }

    crate::ui::detail_page(
        app,
        text.title,
        crate::app::Nav::Home(crate::app::Tab::Settings),
        content.into(),
    )
}
