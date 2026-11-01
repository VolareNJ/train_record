#[derive(Debug, Clone, Default)]
pub struct State
{
    pub data: Option<crate::model::Today>,
    pub loading: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone)]
pub enum Message
{
    Refresh,
    Loaded(Result<crate::model::Today, crate::api::ApiError>),
}

pub fn update(app: &mut crate::app::App, message: Message) -> iced::Task<crate::app::Message>
{
    match message
    {
        Message::Refresh =>
        {
            app.today.loading = true;
            app.today.error = None;
            let api = app.api.clone();
            iced::Task::perform(async move { api.today().await }, |result| {
                crate::app::Message::Today(Message::Loaded(result))
            })
        },
        Message::Loaded(result) => match result
        {
            Ok(today) =>
            {
                app.today.loading = false;
                app.today.error = None;
                app.today.data = Some(today);
                iced::Task::none()
            },
            Err(error) =>
            {
                app.today.loading = false;
                if matches!(error, crate::api::ApiError::Unauthorized)
                {
                    crate::app::report_error(app, error)
                }
                else
                {
                    app.today.error = Some(error.to_string());
                    iced::Task::none()
                }
            },
        },
    }
}

pub fn view(app: &crate::app::App) -> crate::ui::El<'_>
{
    let text = crate::copy::today::text(app.lang);
    let common = crate::copy::common(app.lang);
    if app.today.loading && app.today.data.is_none()
    {
        return crate::ui::loading(common.loading);
    }
    if let Some(error) = &app.today.error
        && app.today.data.is_none()
    {
        return crate::ui::error_card(
            app.lang,
            error,
            Some(crate::ui::secondary_button(
                common.retry,
                crate::app::Message::Today(Message::Refresh),
            )),
        );
    }
    let Some(today) = &app.today.data
    else
    {
        return crate::ui::empty_card(
            text.plan_missing,
            text.plan_missing_hint,
            Some(crate::ui::secondary_button(
                common.retry,
                crate::app::Message::Today(Message::Refresh),
            )),
        );
    };

    let Some(phase) = &today.phase
    else
    {
        return crate::ui::empty_card(
            text.phase_missing,
            text.phase_missing_hint,
            Some(crate::ui::secondary_button(
                text.manage_phase,
                crate::app::Message::Nav(crate::app::Nav::Phases),
            )),
        );
    };

    let header = crate::ui::card(
        iced::widget::column![
            iced::widget::row![
                iced::widget::text(phase.name.as_str())
                    .size(22)
                    .font(crate::style::bold())
                    .color(crate::style::TEXT),
                iced::widget::space().width(iced::Length::Fill),
                crate::ui::badge(today.date.as_str(), crate::style::PRIMARY_LIGHT),
            ]
            .align_y(iced::Alignment::Center),
            iced::widget::text(common.days_text(phase.days))
                .size(14)
                .color(crate::style::MUTED),
        ]
        .spacing(6),
    );

    let mut content = iced::widget::column![header].spacing(12);

    let Some(plan) = &today.plan
    else
    {
        content = content.push(crate::ui::empty_card(
            text.plan_missing,
            text.plan_missing_hint,
            Some(crate::ui::primary_button(
                text.create_plan,
                crate::app::Message::OpenNewPlan(phase.id, phase.name.clone()),
            )),
        ));
        return content.into();
    };

    if !plan.note.trim().is_empty()
    {
        content = content.push(crate::ui::flat_card(
            iced::widget::column![
                iced::widget::text(text.plan_note)
                    .size(12)
                    .color(crate::style::MUTED),
                iced::widget::text(plan.note.as_str())
                    .size(14)
                    .color(crate::style::TEXT),
            ]
            .spacing(4),
        ));
    }

    content = content.push(
        iced::widget::row![
            crate::ui::section_title(text.today_plan),
            iced::widget::space().width(iced::Length::Fill),
            crate::ui::ghost_button(
                text.manage_plan,
                crate::app::Message::OpenPlans(phase.id, phase.name.clone()),
            ),
        ]
        .align_y(iced::Alignment::Center),
    );

    let mut groups: Vec<(String, Vec<&crate::model::TodayItem>)> = Vec::new();
    for item in &plan.items
    {
        match groups.iter_mut().find(|(part, _)| *part == item.body_part)
        {
            Some((_, items)) => items.push(item),
            None => groups.push((item.body_part.clone(), vec![item])),
        }
    }
    groups.sort_by_key(|(part, _)| crate::weight::part_rank(part));

    for (part, items) in groups
    {
        content = content.push(crate::ui::section_title(crate::i18n::part_label(
            app.lang, &part,
        )));
        for item in items
        {
            let status = match &item.last_record
            {
                Some(record) if record.completed =>
                {
                    crate::ui::badge(text.trained, crate::style::SUCCESS)
                },
                Some(_) => crate::ui::badge(text.draft, crate::style::WARN),
                None => crate::ui::badge(text.untrained, crate::style::MUTED),
            };
            let observed = item
                .plan_weight
                .map(|weight| {
                    crate::weight::observed_label(
                        &item.default_mode,
                        weight,
                        item.bar_weight,
                        app.user.as_ref().and_then(|user| user.body_weight),
                        &item.default_unit,
                    )
                })
                .unwrap_or_else(|| common.not_set.to_string());
            let plan_line = format!(
                "{} {}  {}",
                common.plan,
                common.sets_reps(item.plan_sets, item.plan_reps),
                observed
            );
            let mut body = iced::widget::column![
                iced::widget::row![
                    iced::widget::text(item.exercise_name.as_str())
                        .size(17)
                        .font(crate::style::bold())
                        .color(crate::style::TEXT),
                    iced::widget::space().width(iced::Length::Fill),
                    status,
                ]
                .align_y(iced::Alignment::Center),
                iced::widget::text(plan_line)
                    .size(13)
                    .color(crate::style::MUTED),
            ]
            .spacing(6);
            match &item.last_record
            {
                Some(record) =>
                {
                    body = body.push(
                        iced::widget::text(format!(
                            "{}: {}  {}",
                            text.last_record,
                            common.volume(record.sets, record.reps, record.weight),
                            if record.feeling.trim().is_empty()
                            {
                                String::new()
                            }
                            else
                            {
                                format!("| {}", record.feeling)
                            }
                        ))
                        .size(13)
                        .color(crate::style::TEXT),
                    );
                    if !record.strategy.trim().is_empty()
                    {
                        body = body.push(
                            iced::widget::text(format!(
                                "{}: {}",
                                text.strategy_hint, record.strategy
                            ))
                            .size(13)
                            .color(crate::style::WARN),
                        );
                    }
                },
                None =>
                {
                    body = body.push(
                        iced::widget::text(text.no_last)
                            .size(12)
                            .color(crate::style::MUTED),
                    );
                },
            }
            content = content.push(
                iced::widget::button(body)
                    .width(iced::Length::Fill)
                    .padding(14)
                    .style(crate::style::list_button)
                    .on_press(crate::app::Message::OpenRecord(plan.id, item.id)),
            );
        }
    }

    content = content.push(
        iced::widget::text(text.tap_to_log)
            .size(12)
            .color(crate::style::MUTED),
    );
    content.into()
}
