// 重量展示的纯函数：实际总重（kg）→ 观测强度文本。
// 与 server/src/handlers/record.rs::mode_display 及
// server/static/weight_converter.js 的逆换算保持一致。

pub const BODY_PARTS: [&str; 6] = ["腿", "背", "胸", "核心", "手臂", "肩"];

fn unit_value(kg: f64, unit: &str) -> String
{
    if unit == "lb"
    {
        format!("{:.0}lb", (kg / 0.4536).round())
    }
    else
    {
        format!("{kg}kg")
    }
}

pub fn observed_label(
    mode: &str,
    weight: f64,
    bar_weight: f64,
    body_weight: Option<f64>,
    unit: &str,
) -> String
{
    match mode
    {
        "bar" =>
        {
            let plate = ((weight - bar_weight) / 2.0).max(0.0);
            format!("bar({}kg, {})", bar_weight, unit_value(plate, unit))
        },
        "support" =>
        {
            let body = body_weight.unwrap_or(0.0);
            let support = (body - weight).max(0.0);
            format!("support({body}kg - {})", unit_value(support, unit))
        },
        _ => format!("std({})", unit_value(weight, unit)),
    }
}

pub fn part_rank(part: &str) -> usize
{
    BODY_PARTS
        .iter()
        .position(|item| *item == part)
        .unwrap_or(usize::MAX)
}
