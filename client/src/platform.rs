const LANG_KEY: &str = "train_record.lang";
#[cfg(not(target_arch = "wasm32"))]
const TOKEN_KEY: &str = "train_record.token";

#[cfg(target_arch = "wasm32")]
fn storage_get(key: &str) -> Option<String>
{
    web_sys::window()
        .and_then(|window| window.local_storage().ok().flatten())
        .and_then(|storage| storage.get_item(key).ok().flatten())
}

#[cfg(target_arch = "wasm32")]
fn storage_set(key: &str, value: &str)
{
    if let Some(storage) =
        web_sys::window().and_then(|window| window.local_storage().ok().flatten())
    {
        let _ = storage.set_item(key, value);
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn token_path() -> Option<std::path::PathBuf>
{
    let home = std::env::var("HOME").ok()?;
    Some(
        std::path::Path::new(&home)
            .join(".config")
            .join("train_record_client")
            .join("token"),
    )
}

#[cfg(not(target_arch = "wasm32"))]
fn storage_get(key: &str) -> Option<String>
{
    match key
    {
        LANG_KEY => std::fs::read_to_string(
            std::path::Path::new(&std::env::var("HOME").ok()?)
                .join(".config")
                .join("train_record_client")
                .join("lang"),
        )
        .ok()
        .map(|value| value.trim().to_string()),
        TOKEN_KEY => std::fs::read_to_string(token_path()?)
            .ok()
            .map(|value| value.trim().to_string()),
        _ => None,
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn storage_set(key: &str, value: &str)
{
    let Some(path) = token_path()
    else
    {
        return;
    };
    let Some(dir) = path.parent()
    else
    {
        return;
    };
    let _ = std::fs::create_dir_all(dir);
    let target = match key
    {
        LANG_KEY => dir.join("lang"),
        TOKEN_KEY => path,
        _ => return,
    };
    let _ = std::fs::write(target, value);
}

#[cfg(not(target_arch = "wasm32"))]
fn storage_remove(key: &str)
{
    if key == TOKEN_KEY
        && let Some(path) = token_path()
    {
        let _ = std::fs::remove_file(path);
    }
}

pub fn base_url() -> String
{
    #[cfg(target_arch = "wasm32")]
    {
        web_sys::window()
            .and_then(|window| window.location().origin().ok())
            .unwrap_or_default()
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        std::env::var("TRAIN_RECORD_API").unwrap_or_else(|_| "http://127.0.0.1:8080".to_string())
    }
}

pub fn load_lang() -> crate::i18n::Lang
{
    match storage_get(LANG_KEY).as_deref()
    {
        Some("en") => crate::i18n::Lang::En,
        _ => crate::i18n::Lang::Zh,
    }
}

pub fn save_lang(lang: crate::i18n::Lang)
{
    storage_set(LANG_KEY, if lang.is_zh() { "zh" } else { "en" });
}

#[cfg(target_arch = "wasm32")]
pub fn load_token() -> Option<String>
{
    None
}

#[cfg(not(target_arch = "wasm32"))]
pub fn load_token() -> Option<String>
{
    storage_get(TOKEN_KEY).filter(|token| !token.is_empty())
}

#[cfg(target_arch = "wasm32")]
pub fn save_token(_token: &str) {}

#[cfg(not(target_arch = "wasm32"))]
pub fn save_token(token: &str)
{
    if !token.is_empty()
    {
        storage_set(TOKEN_KEY, token);
    }
}

#[cfg(target_arch = "wasm32")]
pub fn clear_token() {}

#[cfg(not(target_arch = "wasm32"))]
pub fn clear_token()
{
    storage_remove(TOKEN_KEY);
}

pub fn today() -> String
{
    #[cfg(target_arch = "wasm32")]
    {
        let date = js_sys::Date::new_0();
        format!(
            "{:04}-{:02}-{:02}",
            date.get_full_year(),
            date.get_month() + 1,
            date.get_date()
        )
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        time::OffsetDateTime::now_local()
            .unwrap_or_else(|_| time::OffsetDateTime::now_utc())
            .date()
            .to_string()
    }
}

pub fn current_month() -> (i32, u32)
{
    let date = today();
    parse_year_month(&date).unwrap_or((2026, 1))
}

pub fn parse_year_month(date: &str) -> Option<(i32, u32)>
{
    let mut parts = date.split('-');
    let year = parts.next()?.parse().ok()?;
    let month = parts.next()?.parse().ok()?;
    if (1..=12).contains(&month)
    {
        Some((year, month))
    }
    else
    {
        None
    }
}

pub fn month_key(year: i32, month: u32) -> String
{
    format!("{year:04}-{month:02}")
}

pub fn date_key(year: i32, month: u32, day: u32) -> String
{
    format!("{year:04}-{month:02}-{day:02}")
}

pub fn shift_month(year: i32, month: u32, delta: i32) -> (i32, u32)
{
    let index = year * 12 + month as i32 - 1 + delta;
    (index.div_euclid(12), index.rem_euclid(12) as u32 + 1)
}

pub fn days_in_month(year: i32, month: u32) -> u32
{
    match month
    {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if is_leap_year(year) => 29,
        2 => 28,
        _ => 30,
    }
}

fn is_leap_year(year: i32) -> bool
{
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

pub fn weekday_index(year: i32, month: u32, day: u32) -> u32
{
    let (year, month) = if month < 3
    {
        (year - 1, month + 12)
    }
    else
    {
        (year, month)
    };
    let k = year.rem_euclid(100) as i64;
    let j = year.div_euclid(100) as i64;
    let h = (day as i64 + 13 * (month as i64 + 1) / 5 + k + k / 4 + j / 4 + 5 * j).rem_euclid(7);
    ((h + 5) % 7) as u32
}

pub fn valid_date(value: &str) -> bool
{
    if value.len() != 10
    {
        return false;
    }
    let mut parts = value.split('-');
    let (Some(year), Some(month), Some(day), None) =
        (parts.next(), parts.next(), parts.next(), parts.next())
    else
    {
        return false;
    };
    let (Ok(year), Ok(month), Ok(day)) = (
        year.parse::<i32>(),
        month.parse::<u32>(),
        day.parse::<u32>(),
    )
    else
    {
        return false;
    };
    year > 1900 && (1..=12).contains(&month) && day >= 1 && day <= days_in_month(year, month)
}

#[cfg(test)]
mod tests
{
    #[test]
    fn month_shift_wraps()
    {
        assert_eq!(crate::platform::shift_month(2026, 1, -1), (2025, 12));
        assert_eq!(crate::platform::shift_month(2026, 12, 1), (2027, 1));
    }

    #[test]
    fn leap_years()
    {
        assert_eq!(crate::platform::days_in_month(2024, 2), 29);
        assert_eq!(crate::platform::days_in_month(2026, 2), 28);
        assert_eq!(crate::platform::days_in_month(2000, 2), 29);
    }

    #[test]
    fn weekdays_are_monday_first()
    {
        assert_eq!(crate::platform::weekday_index(2026, 10, 29), 3);
        assert_eq!(crate::platform::weekday_index(2026, 10, 26), 0);
        assert_eq!(crate::platform::weekday_index(2026, 11, 1), 6);
    }

    #[test]
    fn date_validation()
    {
        assert!(crate::platform::valid_date("2026-10-29"));
        assert!(!crate::platform::valid_date("2026-02-30"));
        assert!(!crate::platform::valid_date("2026-1-1"));
        assert!(!crate::platform::valid_date(""));
    }
}

pub fn open_url(url: &str)
{
    #[cfg(target_arch = "wasm32")]
    {
        if let Some(window) = web_sys::window()
        {
            let _ = window.open_with_url_and_target(url, "_blank");
        }
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = url;
    }
}
