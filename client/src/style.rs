pub const BG: iced::Color = iced::Color::from_rgb8(0x0e, 0x11, 0x17);
pub const SURFACE: iced::Color = iced::Color::from_rgb8(0x17, 0x1c, 0x25);
pub const SURFACE_2: iced::Color = iced::Color::from_rgb8(0x20, 0x27, 0x33);
pub const SURFACE_3: iced::Color = iced::Color::from_rgb8(0x2a, 0x33, 0x42);
pub const BORDER: iced::Color = iced::Color::from_rgb8(0x31, 0x3c, 0x4d);
pub const TEXT: iced::Color = iced::Color::from_rgb8(0xe9, 0xee, 0xf7);
pub const MUTED: iced::Color = iced::Color::from_rgb8(0x9a, 0xa7, 0xba);
pub const PRIMARY: iced::Color = iced::Color::from_rgb8(0x4d, 0x8d, 0xff);
pub const PRIMARY_LIGHT: iced::Color = iced::Color::from_rgb8(0x6c, 0xa4, 0xff);
pub const PRIMARY_DARK: iced::Color = iced::Color::from_rgb8(0x33, 0x6d, 0xd6);
pub const SUCCESS: iced::Color = iced::Color::from_rgb8(0x35, 0xc0, 0x7f);
pub const DANGER: iced::Color = iced::Color::from_rgb8(0xff, 0x5d, 0x6c);
pub const WARN: iced::Color = iced::Color::from_rgb8(0xf5, 0xb5, 0x44);

pub const FONT_NAME: &str = "Carlito";

pub fn regular() -> iced::Font
{
    iced::Font::with_name(FONT_NAME)
}

pub fn bold() -> iced::Font
{
    iced::Font {
        weight: iced::font::Weight::Semibold,
        ..iced::Font::with_name(FONT_NAME)
    }
}

pub fn primary_button(
    _theme: &iced::Theme,
    status: iced::widget::button::Status,
) -> iced::widget::button::Style
{
    let background = match status
    {
        iced::widget::button::Status::Hovered => PRIMARY_LIGHT,
        iced::widget::button::Status::Pressed => PRIMARY_DARK,
        iced::widget::button::Status::Disabled => SURFACE_2,
        iced::widget::button::Status::Active => PRIMARY,
    };
    iced::widget::button::Style {
        background: Some(iced::Background::Color(background)),
        text_color: if matches!(status, iced::widget::button::Status::Disabled)
        {
            MUTED
        }
        else
        {
            iced::Color::WHITE
        },
        border: iced::Border {
            radius: 12.0.into(),
            ..iced::Border::default()
        },
        shadow: iced::Shadow::default(),
        snap: true,
    }
}

pub fn secondary_button(
    _theme: &iced::Theme,
    status: iced::widget::button::Status,
) -> iced::widget::button::Style
{
    let background = match status
    {
        iced::widget::button::Status::Hovered => SURFACE_3,
        iced::widget::button::Status::Pressed => SURFACE_2,
        _ => SURFACE,
    };
    iced::widget::button::Style {
        background: Some(iced::Background::Color(background)),
        text_color: TEXT,
        border: iced::Border {
            color: BORDER,
            width: 1.0,
            radius: 12.0.into(),
        },
        shadow: iced::Shadow::default(),
        snap: true,
    }
}

pub fn ghost_button(
    _theme: &iced::Theme,
    status: iced::widget::button::Status,
) -> iced::widget::button::Style
{
    iced::widget::button::Style {
        background: match status
        {
            iced::widget::button::Status::Hovered => Some(iced::Background::Color(SURFACE_3)),
            iced::widget::button::Status::Pressed => Some(iced::Background::Color(SURFACE_2)),
            _ => None,
        },
        text_color: MUTED,
        border: iced::Border {
            radius: 10.0.into(),
            ..iced::Border::default()
        },
        shadow: iced::Shadow::default(),
        snap: true,
    }
}

pub fn danger_button(
    _theme: &iced::Theme,
    status: iced::widget::button::Status,
) -> iced::widget::button::Style
{
    let background = match status
    {
        iced::widget::button::Status::Hovered => iced::Color::from_rgb8(0xff, 0x7a, 0x86),
        iced::widget::button::Status::Pressed => iced::Color::from_rgb8(0xd9, 0x45, 0x53),
        _ => DANGER,
    };
    iced::widget::button::Style {
        background: Some(iced::Background::Color(background)),
        text_color: iced::Color::WHITE,
        border: iced::Border {
            radius: 12.0.into(),
            ..iced::Border::default()
        },
        shadow: iced::Shadow::default(),
        snap: true,
    }
}

pub fn nav_button(
    active: bool,
) -> impl Fn(&iced::Theme, iced::widget::button::Status) -> iced::widget::button::Style + 'static
{
    move |_theme, status| iced::widget::button::Style {
        background: if active
        {
            Some(iced::Background::Color(SURFACE_2))
        }
        else
        {
            match status
            {
                iced::widget::button::Status::Hovered | iced::widget::button::Status::Pressed =>
                {
                    Some(iced::Background::Color(SURFACE))
                },
                _ => None,
            }
        },
        text_color: if active { PRIMARY_LIGHT } else { MUTED },
        border: iced::Border {
            radius: 12.0.into(),
            ..iced::Border::default()
        },
        shadow: iced::Shadow::default(),
        snap: true,
    }
}

pub fn card(_theme: &iced::Theme) -> iced::widget::container::Style
{
    iced::widget::container::Style {
        text_color: Some(TEXT),
        background: Some(iced::Background::Color(SURFACE)),
        border: iced::Border {
            color: BORDER,
            width: 1.0,
            radius: 16.0.into(),
        },
        shadow: iced::Shadow::default(),
        snap: true,
    }
}

pub fn card_flat(_theme: &iced::Theme) -> iced::widget::container::Style
{
    iced::widget::container::Style {
        text_color: Some(TEXT),
        background: Some(iced::Background::Color(SURFACE_2)),
        border: iced::Border {
            radius: 12.0.into(),
            ..iced::Border::default()
        },
        shadow: iced::Shadow::default(),
        snap: true,
    }
}

pub fn badge(
    color: iced::Color,
) -> impl Fn(&iced::Theme) -> iced::widget::container::Style + 'static
{
    move |_theme| iced::widget::container::Style {
        text_color: Some(color),
        background: Some(iced::Background::Color(iced::Color::from_rgba(
            color.r, color.g, color.b, 0.14,
        ))),
        border: iced::Border {
            color: iced::Color::from_rgba(color.r, color.g, color.b, 0.35),
            width: 1.0,
            radius: 8.0.into(),
        },
        shadow: iced::Shadow::default(),
        snap: true,
    }
}

pub fn text_input(
    _theme: &iced::Theme,
    status: iced::widget::text_input::Status,
) -> iced::widget::text_input::Style
{
    let border_color = match status
    {
        iced::widget::text_input::Status::Focused { .. } => PRIMARY,
        iced::widget::text_input::Status::Hovered => BORDER,
        _ => BORDER,
    };
    iced::widget::text_input::Style {
        background: iced::Background::Color(SURFACE_2),
        border: iced::Border {
            color: border_color,
            width: 1.0,
            radius: 10.0.into(),
        },
        icon: MUTED,
        placeholder: MUTED,
        value: TEXT,
        selection: PRIMARY_DARK,
    }
}

pub fn pick_list(
    _theme: &iced::Theme,
    status: iced::widget::pick_list::Status,
) -> iced::widget::pick_list::Style
{
    let border_color = match status
    {
        iced::widget::pick_list::Status::Hovered => PRIMARY,
        iced::widget::pick_list::Status::Opened { .. } => PRIMARY,
        iced::widget::pick_list::Status::Active => BORDER,
    };
    iced::widget::pick_list::Style {
        text_color: TEXT,
        placeholder_color: MUTED,
        handle_color: MUTED,
        background: iced::Background::Color(SURFACE_2),
        border: iced::Border {
            color: border_color,
            width: 1.0,
            radius: 10.0.into(),
        },
    }
}

pub fn app_background(_theme: &iced::Theme) -> iced::widget::container::Style
{
    iced::widget::container::Style {
        text_color: Some(TEXT),
        background: Some(iced::Background::Color(BG)),
        ..iced::widget::container::Style::default()
    }
}

pub fn list_button(
    _theme: &iced::Theme,
    status: iced::widget::button::Status,
) -> iced::widget::button::Style
{
    let background = match status
    {
        iced::widget::button::Status::Hovered => SURFACE_2,
        iced::widget::button::Status::Pressed => SURFACE_3,
        _ => SURFACE,
    };
    iced::widget::button::Style {
        background: Some(iced::Background::Color(background)),
        text_color: TEXT,
        border: iced::Border {
            color: BORDER,
            width: 1.0,
            radius: 16.0.into(),
        },
        shadow: iced::Shadow::default(),
        snap: true,
    }
}

pub fn pick_menu(_theme: &iced::Theme) -> iced::overlay::menu::Style
{
    iced::overlay::menu::Style {
        background: iced::Background::Color(SURFACE_2),
        border: iced::Border {
            color: BORDER,
            width: 1.0,
            radius: 10.0.into(),
        },
        text_color: TEXT,
        selected_text_color: iced::Color::WHITE,
        selected_background: iced::Background::Color(PRIMARY_DARK),
        shadow: iced::Shadow::default(),
    }
}
