#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lang
{
    Zh,
    En,
}

impl Lang
{
    pub fn toggle(self) -> Self
    {
        match self
        {
            Lang::Zh => Lang::En,
            Lang::En => Lang::Zh,
        }
    }

    pub fn label(self) -> &'static str
    {
        match self
        {
            Lang::Zh => "中文",
            Lang::En => "English",
        }
    }

    pub fn is_zh(self) -> bool
    {
        matches!(self, Lang::Zh)
    }
}

const PARTS: [(&str, &str); 6] = [
    ("胸", "Chest"),
    ("背", "Back"),
    ("腿", "Legs"),
    ("肩", "Shoulders"),
    ("手臂", "Arms"),
    ("核心", "Core"),
];

pub fn part_label(lang: Lang, code: &str) -> String
{
    match lang
    {
        Lang::Zh => code.to_string(),
        Lang::En => PARTS
            .iter()
            .find(|(zh, _)| *zh == code)
            .map(|(_, en)| (*en).to_string())
            .unwrap_or_else(|| code.to_string()),
    }
}

pub fn part_code(lang: Lang, label: &str) -> String
{
    match lang
    {
        Lang::Zh => label.to_string(),
        Lang::En => PARTS
            .iter()
            .find(|(_, en)| *en == label)
            .map(|(zh, _)| (*zh).to_string())
            .unwrap_or_else(|| label.to_string()),
    }
}

pub fn mode_label(lang: Lang, code: &str) -> String
{
    match (lang, code)
    {
        (Lang::Zh, "bar") => "杠铃".to_string(),
        (Lang::Zh, "support") => "自重减支撑".to_string(),
        (Lang::Zh, _) => "器械 / 自重".to_string(),
        (Lang::En, "bar") => "Barbell".to_string(),
        (Lang::En, "support") => "Bodyweight minus assistance".to_string(),
        (Lang::En, _) => "Machine / bodyweight".to_string(),
    }
}
