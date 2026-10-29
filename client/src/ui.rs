pub type El<'a> = iced::Element<'a, crate::app::Message>;

pub fn home_page<'a>(app: &'a crate::app::App, title: &'a str, content: El<'a>) -> El<'a>
{
    let top = top_bar(app, title, None);
    let body = iced::widget::scrollable(
        iced::widget::container(
            iced::widget::container(content)
                .width(iced::Length::Fill)
                .max_width(640.0)
                .padding([14, 16]),
        )
        .width(iced::Length::Fill)
        .center_x(iced::Length::Fill),
    )
    .height(iced::Length::Fill);
    let mut root = iced::widget::column![top];
    if let Some(toast) = &app.toast
    {
        root = root.push(toast_bar(toast));
    }
    root = root.push(body).push(bottom_nav(app));
    iced::widget::container(root)
        .width(iced::Length::Fill)
        .height(iced::Length::Fill)
        .style(crate::style::app_background)
        .into()
}

pub fn detail_page<'a>(
    app: &'a crate::app::App,
    title: &'a str,
    back: crate::app::Nav,
    content: El<'a>,
) -> El<'a>
{
    let top = top_bar(app, title, Some(back));
    let body = iced::widget::scrollable(
        iced::widget::container(
            iced::widget::container(content)
                .width(iced::Length::Fill)
                .max_width(640.0)
                .padding([14, 16]),
        )
        .width(iced::Length::Fill)
        .center_x(iced::Length::Fill),
    )
    .height(iced::Length::Fill);
    let mut root = iced::widget::column![top];
    if let Some(toast) = &app.toast
    {
        root = root.push(toast_bar(toast));
    }
    root = root.push(body);
    iced::widget::container(root)
        .width(iced::Length::Fill)
        .height(iced::Length::Fill)
        .style(crate::style::app_background)
        .into()
}

fn top_bar<'a>(app: &'a crate::app::App, title: &'a str, back: Option<crate::app::Nav>) -> El<'a>
{
    let common = crate::copy::common(app.lang);
    let left: El<'a> = match back
    {
        Some(nav) => iced::widget::button(iced::widget::text("<").size(18))
            .padding([6, 12])
            .style(crate::style::ghost_button)
            .on_press(crate::app::Message::Nav(nav))
            .into(),
        None => iced::widget::space().width(0).into(),
    };
    iced::widget::container(
        iced::widget::container(
            iced::widget::row![
                left,
                iced::widget::text(title)
                    .font(crate::style::bold())
                    .size(20)
                    .color(crate::style::TEXT),
                iced::widget::space().width(iced::Length::Fill),
                iced::widget::button(iced::widget::text(common.language).size(13))
                    .padding([6, 10])
                    .style(crate::style::ghost_button)
                    .on_press(crate::app::Message::ToggleLang),
            ]
            .width(iced::Length::Fill)
            .spacing(8)
            .align_y(iced::Alignment::Center)
            .padding([10, 12]),
        )
        .width(iced::Length::Fill)
        .max_width(672.0),
    )
    .width(iced::Length::Fill)
    .center_x(iced::Length::Fill)
    .into()
}

fn toast_bar<'a>(message: &'a str) -> El<'a>
{
    iced::widget::container(
        iced::widget::text(message)
            .size(14)
            .color(crate::style::TEXT)
            .width(iced::Length::Fill),
    )
    .width(iced::Length::Fill)
    .padding([8, 14])
    .style(|_| iced::widget::container::Style {
        background: Some(iced::Background::Color(crate::style::SURFACE_3)),
        text_color: Some(crate::style::TEXT),
        ..iced::widget::container::Style::default()
    })
    .into()
}

fn bottom_nav<'a>(app: &'a crate::app::App) -> El<'a>
{
    let common = crate::copy::common(app.lang);
    let tabs = [
        (common.today, crate::app::Tab::Today),
        (common.history, crate::app::Tab::History),
        (common.library, crate::app::Tab::Library),
        (common.settings, crate::app::Tab::Settings),
    ];
    let mut row = iced::widget::row![].spacing(4).padding([6, 8]);
    for (label, tab) in tabs
    {
        row = row.push(
            iced::widget::button(iced::widget::text(label).size(14))
                .width(iced::Length::Fill)
                .height(48)
                .style(crate::style::nav_button(app.tab == tab))
                .on_press(crate::app::Message::Tab(tab)),
        );
    }
    iced::widget::container(
        iced::widget::container(row)
            .width(iced::Length::Fill)
            .max_width(672.0),
    )
    .width(iced::Length::Fill)
    .center_x(iced::Length::Fill)
    .style(|_| iced::widget::container::Style {
        background: Some(iced::Background::Color(crate::style::BG)),
        border: iced::Border {
            color: crate::style::BORDER,
            width: 1.0,
            radius: 0.0.into(),
        },
        ..iced::widget::container::Style::default()
    })
    .into()
}

pub fn section_title<'a>(label: impl Into<String>) -> El<'a>
{
    iced::widget::text(label.into())
        .font(crate::style::bold())
        .size(16)
        .color(crate::style::TEXT)
        .into()
}

pub fn muted<'a>(label: impl Into<String>) -> El<'a>
{
    iced::widget::text(label.into())
        .size(13)
        .color(crate::style::MUTED)
        .into()
}

pub fn card<'a>(content: impl Into<El<'a>>) -> El<'a>
{
    iced::widget::container(content)
        .width(iced::Length::Fill)
        .padding(14)
        .style(crate::style::card)
        .into()
}

pub fn flat_card<'a>(content: impl Into<El<'a>>) -> El<'a>
{
    iced::widget::container(content)
        .width(iced::Length::Fill)
        .padding(12)
        .style(crate::style::card_flat)
        .into()
}

pub fn primary_button<'a>(label: &'a str, message: crate::app::Message) -> El<'a>
{
    iced::widget::button(
        iced::widget::text(label)
            .size(16)
            .font(crate::style::bold()),
    )
    .width(iced::Length::Fill)
    .padding([12, 16])
    .style(crate::style::primary_button)
    .on_press(message)
    .into()
}

pub fn primary_button_enabled<'a>(label: &'a str, message: Option<crate::app::Message>) -> El<'a>
{
    iced::widget::button(
        iced::widget::text(label)
            .size(16)
            .font(crate::style::bold()),
    )
    .width(iced::Length::Fill)
    .padding([12, 16])
    .style(crate::style::primary_button)
    .on_press_maybe(message)
    .into()
}

pub fn secondary_button<'a>(label: &'a str, message: crate::app::Message) -> El<'a>
{
    iced::widget::button(iced::widget::text(label).size(14))
        .padding([9, 12])
        .style(crate::style::secondary_button)
        .on_press(message)
        .into()
}

pub fn danger_button<'a>(label: &'a str, message: crate::app::Message) -> El<'a>
{
    iced::widget::button(iced::widget::text(label).size(14))
        .padding([9, 12])
        .style(crate::style::danger_button)
        .on_press(message)
        .into()
}

pub fn ghost_button<'a>(label: &'a str, message: crate::app::Message) -> El<'a>
{
    iced::widget::button(iced::widget::text(label).size(13))
        .padding([6, 10])
        .style(crate::style::ghost_button)
        .on_press(message)
        .into()
}

pub fn badge<'a>(label: impl Into<String>, color: iced::Color) -> El<'a>
{
    iced::widget::container(iced::widget::text(label.into()).size(12).color(color))
        .padding([3, 8])
        .style(crate::style::badge(color))
        .into()
}

pub fn text_field<'a>(
    label: &'a str,
    placeholder: &'a str,
    value: &'a str,
    on_input: impl Fn(String) -> crate::app::Message + 'a,
    secure: bool,
) -> iced::widget::Column<'a, crate::app::Message>
{
    iced::widget::column![
        iced::widget::text(label)
            .size(13)
            .color(crate::style::MUTED),
        iced::widget::text_input(placeholder, value)
            .on_input(on_input)
            .secure(secure)
            .padding(12)
            .size(16)
            .font(crate::style::regular())
            .style(crate::style::text_input),
    ]
    .spacing(6)
}

pub fn pick_field<'a>(
    label: &'a str,
    options: Vec<String>,
    selected: Option<String>,
    on_select: impl Fn(String) -> crate::app::Message + 'a,
) -> El<'a>
{
    iced::widget::column![
        iced::widget::text(label)
            .size(13)
            .color(crate::style::MUTED),
        iced::widget::pick_list(options, selected, on_select)
            .width(iced::Length::Fill)
            .padding(12)
            .text_size(16)
            .font(crate::style::regular())
            .style(crate::style::pick_list)
            .menu_style(crate::style::pick_menu),
    ]
    .spacing(6)
    .into()
}

pub fn checkbox_field<'a>(
    label: &'a str,
    value: bool,
    on_toggle: impl Fn(bool) -> crate::app::Message + 'a,
) -> El<'a>
{
    iced::widget::checkbox(value)
        .label(label)
        .on_toggle(on_toggle)
        .size(22)
        .text_size(15)
        .font(crate::style::regular())
        .into()
}

pub fn stat_line<'a>(label: impl Into<String>, value: impl Into<String>) -> El<'a>
{
    iced::widget::row![
        iced::widget::text(label.into())
            .size(13)
            .color(crate::style::MUTED)
            .width(iced::Length::Fill),
        iced::widget::text(value.into())
            .size(15)
            .font(crate::style::bold())
            .color(crate::style::TEXT),
    ]
    .spacing(8)
    .into()
}

pub fn empty_card<'a>(title: &'a str, hint: &'a str, action: Option<El<'a>>) -> El<'a>
{
    let mut content = iced::widget::column![
        iced::widget::text(title)
            .size(17)
            .font(crate::style::bold())
            .color(crate::style::TEXT),
        iced::widget::text(hint).size(14).color(crate::style::MUTED),
    ]
    .spacing(8);
    if let Some(action) = action
    {
        content = content.push(action);
    }
    card(content)
}

pub fn error_card<'a>(lang: crate::i18n::Lang, message: &'a str, retry: Option<El<'a>>) -> El<'a>
{
    let mut content = iced::widget::column![
        iced::widget::text(crate::copy::common(lang).error)
            .size(14)
            .font(crate::style::bold())
            .color(crate::style::DANGER),
        iced::widget::text(message)
            .size(13)
            .color(crate::style::MUTED),
    ]
    .spacing(6);
    if let Some(retry) = retry
    {
        content = content.push(retry);
    }
    card(content)
}

pub fn loading<'a>(label: impl Into<String>) -> El<'a>
{
    iced::widget::container(
        iced::widget::text(label.into())
            .size(14)
            .color(crate::style::MUTED),
    )
    .width(iced::Length::Fill)
    .padding(24)
    .into()
}

pub fn gap<'a>(height: f32) -> El<'a>
{
    iced::widget::space().height(height).into()
}
