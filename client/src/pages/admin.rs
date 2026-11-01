#[derive(Debug, Clone, Default)]
pub struct State
{
    pub users: Vec<crate::model::AdminUser>,
    pub username: String,
    pub password: String,
    pub is_admin: bool,
    pub loading: bool,
    pub saving: bool,
    pub error: Option<String>,
    pub success: Option<String>,
}

#[derive(Debug, Clone)]
pub enum Message
{
    Refresh,
    Loaded(Result<Vec<crate::model::AdminUser>, crate::api::ApiError>),
    Username(String),
    Password(String),
    IsAdmin(bool),
    Create,
    Created(Result<crate::model::AdminUser, crate::api::ApiError>),
}

pub fn update(app: &mut crate::app::App, message: Message) -> iced::Task<crate::app::Message>
{
    match message
    {
        Message::Refresh =>
        {
            app.admin.loading = true;
            app.admin.error = None;
            let api = app.api.clone();
            iced::Task::perform(async move { api.admin_users().await }, |result| {
                crate::app::Message::Admin(Message::Loaded(result))
            })
        },
        Message::Loaded(result) => match result
        {
            Ok(users) =>
            {
                app.admin.loading = false;
                app.admin.users = users;
                iced::Task::none()
            },
            Err(error) =>
            {
                app.admin.loading = false;
                crate::app::report_error(app, error)
            },
        },
        Message::Username(value) =>
        {
            app.admin.username = value;
            iced::Task::none()
        },
        Message::Password(value) =>
        {
            app.admin.password = value;
            iced::Task::none()
        },
        Message::IsAdmin(value) =>
        {
            app.admin.is_admin = value;
            iced::Task::none()
        },
        Message::Create =>
        {
            if app.admin.username.trim().is_empty() || app.admin.password.len() < 6
            {
                app.admin.error = Some(crate::copy::admin::text(app.lang).password.to_string());
                return iced::Task::none();
            }
            let api = app.api.clone();
            let username = app.admin.username.trim().to_string();
            let password = app.admin.password.clone();
            let is_admin = app.admin.is_admin;
            app.admin.saving = true;
            app.admin.error = None;
            iced::Task::perform(
                async move { api.admin_create_user(&username, &password, is_admin).await },
                |result| crate::app::Message::Admin(Message::Created(result)),
            )
        },
        Message::Created(result) => match result
        {
            Ok(user) =>
            {
                app.admin.saving = false;
                app.admin.success = Some(crate::copy::admin::text(app.lang).created.to_string());
                app.admin.username.clear();
                app.admin.password.clear();
                app.admin.is_admin = false;
                app.admin.users.push(user);
                iced::Task::none()
            },
            Err(error) =>
            {
                app.admin.saving = false;
                if matches!(error, crate::api::ApiError::Unauthorized)
                {
                    crate::app::report_error(app, error)
                }
                else
                {
                    app.admin.error = Some(error.to_string());
                    iced::Task::none()
                }
            },
        },
    }
}

pub fn view(app: &crate::app::App) -> crate::ui::El<'_>
{
    let text = crate::copy::admin::text(app.lang);
    let common = crate::copy::common(app.lang);
    let mut content = iced::widget::column![].spacing(12);
    content = content.push(crate::ui::section_title(text.users));
    if app.admin.loading
    {
        content = content.push(crate::ui::loading(common.loading));
    }
    if app.admin.users.is_empty() && !app.admin.loading
    {
        content = content.push(crate::ui::muted(text.no_users));
    }
    for user in &app.admin.users
    {
        content = content.push(crate::ui::flat_card(
            iced::widget::row![
                iced::widget::text(format!("#{} {}", user.id, user.username))
                    .size(15)
                    .color(crate::style::TEXT)
                    .width(iced::Length::Fill),
                crate::ui::badge(
                    if user.is_admin
                    {
                        text.role_admin
                    }
                    else
                    {
                        text.role_user
                    },
                    if user.is_admin
                    {
                        crate::style::WARN
                    }
                    else
                    {
                        crate::style::MUTED
                    },
                ),
            ]
            .align_y(iced::Alignment::Center),
        ));
    }
    content = content.push(crate::ui::section_title(text.create));
    content = content.push(crate::ui::text_field(
        text.username,
        "user",
        &app.admin.username,
        |value| crate::app::Message::Admin(Message::Username(value)),
        false,
    ));
    content = content.push(crate::ui::text_field(
        text.password,
        "******",
        &app.admin.password,
        |value| crate::app::Message::Admin(Message::Password(value)),
        true,
    ));
    content = content.push(crate::ui::checkbox_field(
        text.is_admin,
        app.admin.is_admin,
        |value| crate::app::Message::Admin(Message::IsAdmin(value)),
    ));
    if let Some(error) = &app.admin.error
    {
        content = content.push(
            iced::widget::text(error.as_str())
                .size(13)
                .color(crate::style::DANGER),
        );
    }
    if let Some(success) = &app.admin.success
    {
        content = content.push(
            iced::widget::text(success.as_str())
                .size(13)
                .color(crate::style::SUCCESS),
        );
    }
    content = content.push(crate::ui::primary_button_enabled(
        if app.admin.saving
        {
            common.saving
        }
        else
        {
            text.create
        },
        if app.admin.saving
        {
            None
        }
        else
        {
            Some(crate::app::Message::Admin(Message::Create))
        },
    ));
    crate::ui::detail_page(
        app,
        text.title,
        crate::app::Nav::Home(crate::app::Tab::Settings),
        content.into(),
    )
}
