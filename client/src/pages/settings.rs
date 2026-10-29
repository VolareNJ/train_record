#[derive(Debug, Clone, Default)]
pub struct State
{
    pub body_weight: String,
    pub saving: bool,
    pub error: Option<String>,
    pub confirm_logout: bool,
}

impl State
{
    pub fn sync(&mut self, user: &Option<crate::model::User>)
    {
        if let Some(user) = user
        {
            self.body_weight = user
                .body_weight
                .map(|value| format!("{value:.1}"))
                .unwrap_or_default();
        }
    }
}

#[derive(Debug, Clone)]
pub enum Message
{
    BodyWeight(String),
    Save,
    Saved(Result<crate::model::User, crate::api::ApiError>),
    Language(String),
    LogoutConfirm,
    LogoutCancel,
    Logout,
    Users,
    Backup,
}

pub fn update(app: &mut crate::app::App, message: Message) -> iced::Task<crate::app::Message>
{
    let text = crate::copy::settings::text(app.lang);
    match message
    {
        Message::BodyWeight(value) =>
        {
            app.settings.body_weight = value;
            iced::Task::none()
        },
        Message::Save =>
        {
            let value = app.settings.body_weight.trim();
            let weight = if value.is_empty()
            {
                None
            }
            else
            {
                match value.parse::<f64>()
                {
                    Ok(weight) if (0.0..=500.0).contains(&weight) => Some(weight),
                    _ =>
                    {
                        app.settings.error = Some(text.invalid_weight.to_string());
                        return iced::Task::none();
                    },
                }
            };
            let api = app.api.clone();
            app.settings.saving = true;
            app.settings.error = None;
            iced::Task::perform(async move { api.update_me(weight).await }, |result| {
                crate::app::Message::Settings(Message::Saved(result))
            })
        },
        Message::Saved(result) => match result
        {
            Ok(user) =>
            {
                app.user = Some(user);
                app.settings.saving = false;
                app.settings.sync(&app.user);
                app.toast = Some(text.saved.to_string());
                iced::Task::none()
            },
            Err(error) =>
            {
                app.settings.saving = false;
                if matches!(error, crate::api::ApiError::Unauthorized)
                {
                    crate::app::report_error(app, error)
                }
                else
                {
                    app.settings.error = Some(error.to_string());
                    iced::Task::none()
                }
            },
        },
        Message::Language(label) =>
        {
            let lang = if label == crate::i18n::Lang::En.label()
            {
                crate::i18n::Lang::En
            }
            else
            {
                crate::i18n::Lang::Zh
            };
            iced::Task::done(crate::app::Message::SetLang(lang))
        },
        Message::LogoutConfirm =>
        {
            app.settings.confirm_logout = true;
            iced::Task::none()
        },
        Message::LogoutCancel =>
        {
            app.settings.confirm_logout = false;
            iced::Task::none()
        },
        Message::Logout => iced::Task::done(crate::app::Message::Logout),
        Message::Users =>
        {
            crate::platform::open_url(&format!("{}/admin/users", app.api.base_url()));
            iced::Task::none()
        },
        Message::Backup =>
        {
            crate::platform::open_url(&format!("{}/admin/backup", app.api.base_url()));
            iced::Task::none()
        },
    }
}

pub fn view(app: &crate::app::App) -> crate::ui::El<'_>
{
    let text = crate::copy::settings::text(app.lang);
    let common = crate::copy::common(app.lang);
    let mut content = iced::widget::column![].spacing(12);
    let user = app.user.as_ref();

    let account = iced::widget::column![
        crate::ui::section_title(common.account),
        iced::widget::text(
            user.map(|user| user.username.as_str())
                .unwrap_or(common.unknown)
        )
        .size(20)
        .font(crate::style::bold())
        .color(crate::style::TEXT),
        iced::widget::row![
            crate::ui::badge(
                if user.is_some_and(|user| user.is_admin)
                {
                    common.admin
                }
                else
                {
                    common.account
                },
                if user.is_some_and(|user| user.is_admin)
                {
                    crate::style::WARN
                }
                else
                {
                    crate::style::MUTED
                },
            ),
            iced::widget::text(format!("ID {}", user.map(|user| user.id).unwrap_or(0)))
                .size(12)
                .color(crate::style::MUTED),
        ]
        .spacing(8),
        crate::ui::text_field(
            text.body_weight,
            text.body_weight_hint,
            &app.settings.body_weight,
            |value| crate::app::Message::Settings(Message::BodyWeight(value)),
            false,
        ),
        crate::ui::secondary_button(
            if app.settings.saving
            {
                common.saving
            }
            else
            {
                common.save
            },
            crate::app::Message::Settings(Message::Save),
        ),
    ]
    .spacing(10);
    content = content.push(crate::ui::card(account));

    if let Some(error) = &app.settings.error
    {
        content = content.push(crate::ui::error_card(app.lang, error, None));
    }

    let training = iced::widget::column![
        crate::ui::section_title(text.training),
        crate::ui::secondary_button(
            text.phases_plans,
            crate::app::Message::Nav(crate::app::Nav::Phases),
        ),
    ]
    .spacing(10);
    content = content.push(crate::ui::card(training));

    let language = iced::widget::column![
        crate::ui::section_title(common.language),
        crate::ui::pick_field(
            common.language,
            vec![
                crate::i18n::Lang::Zh.label().to_string(),
                crate::i18n::Lang::En.label().to_string(),
            ],
            Some(app.lang.label().to_string()),
            |label| crate::app::Message::Settings(Message::Language(label)),
        ),
    ]
    .spacing(10);
    content = content.push(crate::ui::card(language));

    let info = iced::widget::column![
        crate::ui::section_title(text.about),
        crate::ui::stat_line(common.server, app.api.base_url()),
        crate::ui::stat_line(text.version, "0.1.0"),
        crate::ui::muted(text.about_text),
    ]
    .spacing(8);
    content = content.push(crate::ui::card(info));

    if app.user.as_ref().is_some_and(|user| user.is_admin)
    {
        content = content.push(crate::ui::card(
            iced::widget::column![
                crate::ui::section_title(text.admin_tools),
                iced::widget::row![
                    crate::ui::secondary_button(
                        text.users,
                        crate::app::Message::Settings(Message::Users),
                    ),
                    crate::ui::secondary_button(
                        text.backup,
                        crate::app::Message::Settings(Message::Backup),
                    ),
                ]
                .spacing(8),
            ]
            .spacing(10),
        ));
    }

    if app.settings.confirm_logout
    {
        content = content.push(crate::ui::card(
            iced::widget::column![
                iced::widget::text(text.sign_out_confirm)
                    .size(14)
                    .color(crate::style::TEXT),
                iced::widget::row![
                    crate::ui::danger_button(
                        common.yes,
                        crate::app::Message::Settings(Message::Logout),
                    ),
                    crate::ui::secondary_button(
                        common.no,
                        crate::app::Message::Settings(Message::LogoutCancel),
                    ),
                ]
                .spacing(8),
            ]
            .spacing(10),
        ));
    }
    else
    {
        content = content.push(crate::ui::danger_button(
            common.logout,
            crate::app::Message::Settings(Message::LogoutConfirm),
        ));
    }

    content.into()
}
