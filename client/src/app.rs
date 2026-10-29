#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab
{
    Today,
    History,
    Library,
    Settings,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Nav
{
    Login,
    Home(Tab),
    Record,
    ExerciseForm,
    ExerciseStats,
    Phases,
    PhaseForm,
    Templates,
    TemplateForm,
    Plans,
    PlanForm,
    Day,
}

#[derive(Debug, Clone)]
pub enum Message
{
    BootChecked(Result<crate::model::User, crate::api::ApiError>),
    ToggleLang,
    SetLang(crate::i18n::Lang),
    Tab(Tab),
    Nav(Nav),
    OpenRecord(i64, i64),
    OpenPlans(i64, String),
    OpenDay(String),
    OpenStats(i64, String),
    Logout,
    LoggedOut,
    Login(crate::pages::login::Message),
    Today(crate::pages::today::Message),
    Record(crate::pages::record::Message),
    History(crate::pages::history::Message),
    Day(crate::pages::day::Message),
    Library(crate::pages::library::Message),
    ExerciseForm(crate::pages::exercise_form::Message),
    Stats(crate::pages::stats::Message),
    Phases(crate::pages::phases::Message),
    PhaseForm(crate::pages::phase_form::Message),
    Templates(crate::pages::templates::Message),
    TemplateForm(crate::pages::template_form::Message),
    Plans(crate::pages::plans::Message),
    PlanForm(crate::pages::plan_form::Message),
    Settings(crate::pages::settings::Message),
}

pub struct App
{
    pub lang: crate::i18n::Lang,
    pub api: crate::api::Api,
    pub route: Nav,
    pub tab: Tab,
    pub toast: Option<String>,
    pub user: Option<crate::model::User>,
    pub login: crate::pages::login::State,
    pub today: crate::pages::today::State,
    pub record: crate::pages::record::State,
    pub history: crate::pages::history::State,
    pub day: crate::pages::day::State,
    pub library: crate::pages::library::State,
    pub exercise_form: crate::pages::exercise_form::State,
    pub stats: crate::pages::stats::State,
    pub phases: crate::pages::phases::State,
    pub phase_form: crate::pages::phase_form::State,
    pub templates: crate::pages::templates::State,
    pub template_form: crate::pages::template_form::State,
    pub plans: crate::pages::plans::State,
    pub plan_form: crate::pages::plan_form::State,
    pub settings: crate::pages::settings::State,
}

pub fn boot() -> (App, iced::Task<Message>)
{
    let lang = crate::platform::load_lang();
    let token = crate::platform::load_token();
    let api = crate::api::Api::new(crate::platform::base_url(), token);
    let api_for_task = api.clone();
    let app = App {
        lang,
        api,
        route: Nav::Login,
        tab: Tab::Today,
        toast: None,
        user: None,
        login: crate::pages::login::State::new(),
        today: crate::pages::today::State::default(),
        record: crate::pages::record::State::default(),
        history: crate::pages::history::State::new(),
        day: crate::pages::day::State::default(),
        library: crate::pages::library::State::default(),
        exercise_form: crate::pages::exercise_form::State::default(),
        stats: crate::pages::stats::State::default(),
        phases: crate::pages::phases::State::default(),
        phase_form: crate::pages::phase_form::State::default(),
        templates: crate::pages::templates::State::default(),
        template_form: crate::pages::template_form::State::default(),
        plans: crate::pages::plans::State::default(),
        plan_form: crate::pages::plan_form::State::default(),
        settings: crate::pages::settings::State::default(),
    };
    let task = iced::Task::perform(async move { api_for_task.me().await }, Message::BootChecked);
    (app, task)
}

pub fn update(app: &mut App, message: Message) -> iced::Task<Message>
{
    app.toast = None;
    match message
    {
        Message::BootChecked(result) => on_boot_checked(app, result),
        Message::ToggleLang =>
        {
            app.lang = app.lang.toggle();
            crate::platform::save_lang(app.lang);
            iced::Task::none()
        },
        Message::SetLang(lang) =>
        {
            app.lang = lang;
            crate::platform::save_lang(lang);
            iced::Task::none()
        },
        Message::Tab(tab) => navigate(app, Nav::Home(tab)),
        Message::Nav(nav) => navigate(app, nav),
        Message::OpenRecord(plan_id, item_id) =>
        {
            open_record(app, plan_id, item_id);
            iced::Task::none()
        },
        Message::OpenPlans(phase_id, phase_name) =>
        {
            app.plans.phase_id = phase_id;
            app.plans.phase_name = phase_name;
            app.plans.plans.clear();
            app.plans.templates.clear();
            app.route = Nav::Plans;
            crate::pages::plans::update(app, crate::pages::plans::Message::Refresh)
        },
        Message::OpenDay(date) => open_day(app, date),
        Message::OpenStats(id, name) => open_stats(app, id, name),
        Message::Logout => sign_out(app),
        Message::LoggedOut => iced::Task::none(),
        Message::Login(message) => crate::pages::login::update(app, message),
        Message::Today(message) => crate::pages::today::update(app, message),
        Message::Record(message) => crate::pages::record::update(app, message),
        Message::History(message) => crate::pages::history::update(app, message),
        Message::Day(message) => crate::pages::day::update(app, message),
        Message::Library(message) => crate::pages::library::update(app, message),
        Message::ExerciseForm(message) => crate::pages::exercise_form::update(app, message),
        Message::Stats(message) => crate::pages::stats::update(app, message),
        Message::Phases(message) => crate::pages::phases::update(app, message),
        Message::PhaseForm(message) => crate::pages::phase_form::update(app, message),
        Message::Templates(message) => crate::pages::templates::update(app, message),
        Message::TemplateForm(message) => crate::pages::template_form::update(app, message),
        Message::Plans(message) => crate::pages::plans::update(app, message),
        Message::PlanForm(message) => crate::pages::plan_form::update(app, message),
        Message::Settings(message) => crate::pages::settings::update(app, message),
    }
}

fn on_boot_checked(
    app: &mut App,
    result: Result<crate::model::User, crate::api::ApiError>,
) -> iced::Task<Message>
{
    match result
    {
        Ok(user) =>
        {
            app.user = Some(user);
            app.route = Nav::Home(Tab::Today);
            app.tab = Tab::Today;
            load_home(app)
        },
        Err(error) =>
        {
            app.route = Nav::Login;
            app.login.checking = false;
            match error
            {
                crate::api::ApiError::Unauthorized =>
                {
                    crate::platform::clear_token();
                },
                other =>
                {
                    app.login.error = Some(other.to_string());
                },
            }
            iced::Task::none()
        },
    }
}

pub fn navigate(app: &mut App, nav: Nav) -> iced::Task<Message>
{
    match nav
    {
        Nav::Login =>
        {
            app.route = Nav::Login;
            iced::Task::none()
        },
        Nav::Home(tab) =>
        {
            app.route = Nav::Home(tab);
            app.tab = tab;
            let mut tasks = Vec::new();
            match tab
            {
                Tab::Today =>
                {
                    if app.today.data.is_none()
                    {
                        tasks.push(crate::pages::today::update(
                            app,
                            crate::pages::today::Message::Refresh,
                        ));
                    }
                },
                Tab::History =>
                {
                    if app.history.calendar.is_none()
                    {
                        tasks.push(crate::pages::history::update(
                            app,
                            crate::pages::history::Message::Refresh,
                        ));
                    }
                },
                Tab::Library =>
                {
                    if app.library.items.is_empty()
                    {
                        tasks.push(crate::pages::library::update(
                            app,
                            crate::pages::library::Message::Refresh,
                        ));
                    }
                },
                Tab::Settings =>
                {
                    app.settings.sync(&app.user);
                },
            }
            iced::Task::batch(tasks)
        },
        Nav::Record => iced::Task::none(),
        Nav::ExerciseForm => iced::Task::none(),
        Nav::ExerciseStats => iced::Task::none(),
        Nav::Phases =>
        {
            app.route = Nav::Phases;
            crate::pages::phases::update(app, crate::pages::phases::Message::Refresh)
        },
        Nav::PhaseForm => iced::Task::none(),
        Nav::Templates =>
        {
            app.route = Nav::Templates;
            crate::pages::templates::update(app, crate::pages::templates::Message::Refresh)
        },
        Nav::TemplateForm => iced::Task::none(),
        Nav::Plans =>
        {
            app.route = Nav::Plans;
            crate::pages::plans::update(app, crate::pages::plans::Message::Refresh)
        },
        Nav::PlanForm => iced::Task::none(),
        Nav::Day =>
        {
            app.route = Nav::Day;
            crate::pages::day::update(app, crate::pages::day::Message::Refresh)
        },
    }
}

pub fn load_home(app: &mut App) -> iced::Task<Message>
{
    let tasks = [
        crate::pages::today::update(app, crate::pages::today::Message::Refresh),
        crate::pages::phases::update(app, crate::pages::phases::Message::Refresh),
        crate::pages::library::update(app, crate::pages::library::Message::Refresh),
    ];
    iced::Task::batch(tasks)
}

pub fn open_record(app: &mut App, plan_id: i64, item_id: i64)
{
    let item = app
        .today
        .data
        .as_ref()
        .and_then(|today| today.plan.as_ref())
        .and_then(|plan| plan.items.iter().find(|item| item.id == item_id).cloned());
    if let Some(item) = item
    {
        app.record = crate::pages::record::State::from_item(app, plan_id, &item);
        app.route = Nav::Record;
    }
    else
    {
        app.toast = Some(crate::copy::common(app.lang).unknown.to_string());
    }
}

pub fn open_day(app: &mut App, date: String) -> iced::Task<Message>
{
    app.day.date = date;
    app.day.records.clear();
    app.day.loading = true;
    app.day.error = None;
    app.route = Nav::Day;
    crate::pages::day::update(app, crate::pages::day::Message::Refresh)
}

pub fn open_stats(app: &mut App, id: i64, name: String) -> iced::Task<Message>
{
    app.stats = crate::pages::stats::State::new(id, name);
    app.route = Nav::ExerciseStats;
    crate::pages::stats::update(app, crate::pages::stats::Message::Refresh)
}

pub fn report_error(app: &mut App, error: crate::api::ApiError) -> iced::Task<Message>
{
    match error
    {
        crate::api::ApiError::Unauthorized =>
        {
            app.user = None;
            app.route = Nav::Login;
            crate::platform::clear_token();
            app.login = crate::pages::login::State::new();
            app.login.checking = false;
            iced::Task::none()
        },
        other =>
        {
            app.toast = Some(other.to_string());
            iced::Task::none()
        },
    }
}

pub fn reset_data(app: &mut App)
{
    app.today = crate::pages::today::State::default();
    app.history = crate::pages::history::State::new();
    app.day = crate::pages::day::State::default();
    app.library = crate::pages::library::State::default();
    app.exercise_form = crate::pages::exercise_form::State::default();
    app.stats = crate::pages::stats::State::default();
    app.phases = crate::pages::phases::State::default();
    app.phase_form = crate::pages::phase_form::State::default();
    app.templates = crate::pages::templates::State::default();
    app.template_form = crate::pages::template_form::State::default();
    app.plans = crate::pages::plans::State::default();
    app.plan_form = crate::pages::plan_form::State::default();
    app.record = crate::pages::record::State::default();
    app.settings = crate::pages::settings::State::default();
}

fn sign_out(app: &mut App) -> iced::Task<Message>
{
    let api = app.api.clone();
    app.user = None;
    reset_data(app);
    app.route = Nav::Login;
    app.tab = Tab::Today;
    app.login = crate::pages::login::State::new();
    app.login.checking = false;
    crate::platform::clear_token();
    app.api = crate::api::Api::new(crate::platform::base_url(), None);
    iced::Task::perform(async move { api.logout().await }, |_| Message::LoggedOut)
}

pub fn view(app: &App) -> crate::ui::El<'_>
{
    match app.route
    {
        Nav::Login => crate::pages::login::view(app),
        Nav::Home(_) => view_home(app),
        Nav::Record => crate::pages::record::view(app),
        Nav::ExerciseForm => crate::pages::exercise_form::view(app),
        Nav::ExerciseStats => crate::pages::stats::view(app),
        Nav::Phases => crate::pages::phases::view(app),
        Nav::PhaseForm => crate::pages::phase_form::view(app),
        Nav::Templates => crate::pages::templates::view(app),
        Nav::TemplateForm => crate::pages::template_form::view(app),
        Nav::Plans => crate::pages::plans::view(app),
        Nav::PlanForm => crate::pages::plan_form::view(app),
        Nav::Day => crate::pages::day::view(app),
    }
}

fn view_home(app: &App) -> crate::ui::El<'_>
{
    let common = crate::copy::common(app.lang);
    match app.tab
    {
        Tab::Today => crate::ui::home_page(app, common.today, crate::pages::today::view(app)),
        Tab::History => crate::ui::home_page(app, common.history, crate::pages::history::view(app)),
        Tab::Library => crate::ui::home_page(app, common.library, crate::pages::library::view(app)),
        Tab::Settings =>
        {
            crate::ui::home_page(app, common.settings, crate::pages::settings::view(app))
        },
    }
}
