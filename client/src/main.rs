mod api;
mod app;
mod copy;
mod i18n;
mod model;
mod pages;
mod platform;
mod style;
mod ui;

fn window_settings() -> iced::window::Settings
{
    #[cfg(target_arch = "wasm32")]
    {
        iced::window::Settings {
            size: iced::Size::new(430.0, 900.0),
            min_size: Some(iced::Size::new(320.0, 520.0)),
            platform_specific: iced::window::settings::PlatformSpecific {
                target: Some("iced".to_string()),
            },
            ..iced::window::Settings::default()
        }
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        iced::window::Settings {
            size: iced::Size::new(430.0, 900.0),
            min_size: Some(iced::Size::new(320.0, 520.0)),
            ..iced::window::Settings::default()
        }
    }
}

fn run() -> iced::Result
{
    iced::application(app::boot, app::update, app::view)
        .title("训练记录")
        .theme(iced::Theme::Dark)
        .default_font(style::regular())
        .font(include_bytes!("../assets/Carlito-Regular.ttf").as_slice())
        .font(include_bytes!("../assets/DroidSansFallbackFull.ttf").as_slice())
        .antialiasing(true)
        .window(window_settings())
        .run()
}

#[cfg(target_arch = "wasm32")]
fn main()
{
    start();
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen(start)]
pub fn start()
{
    console_error_panic_hook::set_once();
    if let Err(error) = run()
    {
        panic!("{error}");
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn main() -> iced::Result
{
    run()
}
