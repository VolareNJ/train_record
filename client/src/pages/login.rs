#[derive(Debug, Clone)]
pub struct State
{
    pub username: String,
    pub password: String,
    pub loading: bool,
    pub checking: bool,
    pub error: Option<String>,
}

impl State
{
    pub fn new() -> Self
    {
        Self {
            username: String::new(),
            password: String::new(),
            loading: false,
            checking: true,
            error: None,
        }
    }
}

#[derive(Debug, Clone)]
pub enum Message
{
    Username(String),
    Password(String),
    Submit,
    Loaded(Result<crate::model::LoginResponse, crate::api::ApiError>),
}

pub fn update(app: &mut crate::app::App, message: Message) -> iced::Task<crate::app::Message>
{
    match message
    {
        Message::Username(value) =>
        {
            app.login.username = value;
            iced::Task::none()
        },
        Message::Password(value) =>
        {
            app.login.password = value;
            iced::Task::none()
        },
        Message::Submit =>
        {
            if app.login.loading
            {
                return iced::Task::none();
            }
            if app.login.username.trim().is_empty() || app.login.password.is_empty()
            {
                app.login.error = Some(crate::copy::common(app.lang).required.to_string());
                return iced::Task::none();
            }
            app.login.loading = true;
            app.login.error = None;
            let api = app.api.clone();
            let username = app.login.username.clone();
            let password = app.login.password.clone();
            iced::Task::perform(
                async move { api.login(username, password).await },
                |result| crate::app::Message::Login(Message::Loaded(result)),
            )
        },
        Message::Loaded(result) => match result
        {
            Ok(response) =>
            {
                crate::platform::save_token(&response.token);
                app.user = Some(response.user);
                crate::app::reset_data(app);
                app.api = crate::api::Api::new(crate::platform::base_url(), Some(response.token));
                app.login = State::new();
                app.login.checking = false;
                app.route = crate::app::Nav::Home(crate::app::Tab::Today);
                app.tab = crate::app::Tab::Today;
                app.toast = Some(crate::copy::common(app.lang).saved.to_string());
                crate::app::load_home(app)
            },
            Err(error) =>
            {
                app.login.loading = false;
                app.login.checking = false;
                app.login.error = Some(error.to_string());
                iced::Task::none()
            },
        },
    }
}

pub fn view(app: &crate::app::App) -> crate::ui::El<'_>
{
    let text = crate::copy::login::text(app.lang);
    let common = crate::copy::common(app.lang);
    let content = if app.login.checking
    {
        iced::widget::column![
            iced::widget::text(common.loading)
                .size(16)
                .color(crate::style::MUTED),
        ]
    }
    else
    {
        let mut form = iced::widget::column![
            iced::widget::text(text.title)
                .size(28)
                .font(crate::style::bold())
                .color(crate::style::TEXT),
            iced::widget::text(text.subtitle)
                .size(14)
                .color(crate::style::MUTED),
            crate::ui::gap(18.0),
            crate::ui::text_field(
                text.username,
                text.username,
                &app.login.username,
                |value| crate::app::Message::Login(Message::Username(value)),
                false,
            ),
            crate::ui::text_field(
                text.password,
                text.password,
                &app.login.password,
                |value| crate::app::Message::Login(Message::Password(value)),
                true,
            ),
            crate::ui::primary_button_enabled(
                if app.login.loading
                {
                    text.signing_in
                }
                else
                {
                    text.submit
                },
                if app.login.loading
                {
                    None
                }
                else
                {
                    Some(crate::app::Message::Login(Message::Submit))
                },
            ),
            iced::widget::text(text.remember)
                .size(12)
                .color(crate::style::MUTED),
        ]
        .spacing(12);
        if let Some(error) = &app.login.error
        {
            form = form.push(
                iced::widget::text(error.as_str())
                    .size(13)
                    .color(crate::style::DANGER),
            );
        }
        form = form.push(
            iced::widget::text(format!("{}: {}", common.server, app.api.base_url()))
                .size(11)
                .color(crate::style::MUTED),
        );
        form
    };

    let card = iced::widget::container(content)
        .width(iced::Length::Fill)
        .max_width(420.0)
        .padding(20)
        .style(crate::style::card);

    let page = iced::widget::column![
        iced::widget::row![
            iced::widget::space().width(iced::Length::Fill),
            iced::widget::button(iced::widget::text(common.language).size(13))
                .padding([6, 10])
                .style(crate::style::ghost_button)
                .on_press(crate::app::Message::ToggleLang),
        ],
        iced::widget::space().height(iced::Length::Fill),
        card,
        iced::widget::space().height(iced::Length::Fill),
        iced::widget::text(text.hint)
            .size(12)
            .color(crate::style::MUTED),
    ]
    .spacing(14)
    .padding(24)
    .width(iced::Length::Fill)
    .max_width(440.0);

    iced::widget::container(
        iced::widget::scrollable(
            iced::widget::container(page)
                .width(iced::Length::Fill)
                .height(iced::Length::Fill)
                .center_x(iced::Length::Fill)
                .center_y(iced::Length::Fill),
        )
        .height(iced::Length::Fill),
    )
    .width(iced::Length::Fill)
    .height(iced::Length::Fill)
    .style(crate::style::app_background)
    .into()
}
