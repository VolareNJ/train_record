#[derive(Debug, Clone, Default)]
pub struct State
{
    pub uploading: bool,
    pub error: Option<String>,
    pub success: Option<String>,
}

#[derive(Debug, Clone)]
pub enum Message
{
    Upload,
    Uploaded(Result<String, String>),
}

pub fn update(app: &mut crate::app::App, message: Message) -> iced::Task<crate::app::Message>
{
    match message
    {
        Message::Upload =>
        {
            app.backup.uploading = true;
            app.backup.error = None;
            app.backup.success = None;
            iced::Task::perform(crate::platform::upload_backup(), |result| {
                crate::app::Message::Backup(Message::Uploaded(result))
            })
        },
        Message::Uploaded(result) => match result
        {
            Ok(message) =>
            {
                app.backup.uploading = false;
                app.backup.success = Some(message);
                iced::Task::none()
            },
            Err(error) =>
            {
                app.backup.uploading = false;
                app.backup.error = Some(error);
                iced::Task::none()
            },
        },
    }
}

pub fn view(app: &crate::app::App) -> crate::ui::El<'_>
{
    let text = crate::copy::admin::text(app.lang);
    let common = crate::copy::common(app.lang);
    let base = app.api.base_url().trim_end_matches('/');
    let mut content = iced::widget::column![].spacing(12);
    content = content.push(crate::ui::card(
        iced::widget::column![
            crate::ui::wide_secondary_button(
                text.download_db,
                crate::app::Message::OpenUrl(format!("{base}/api/v1/admin/backup/download")),
            ),
            crate::ui::wide_secondary_button(
                text.upload_db,
                crate::app::Message::Backup(Message::Upload),
            ),
            crate::ui::wide_secondary_button(
                text.export_csv,
                crate::app::Message::OpenUrl(format!(
                    "{base}/api/v1/admin/backup/export?format=csv"
                )),
            ),
            crate::ui::wide_secondary_button(
                text.export_json,
                crate::app::Message::OpenUrl(format!(
                    "{base}/api/v1/admin/backup/export?format=json"
                )),
            ),
            crate::ui::muted(text.backup_hint),
        ]
        .spacing(10),
    ));
    if app.backup.uploading
    {
        content = content.push(crate::ui::loading(common.loading));
    }
    if let Some(error) = &app.backup.error
    {
        content = content.push(
            iced::widget::text(error.as_str())
                .size(13)
                .color(crate::style::DANGER),
        );
    }
    if let Some(success) = &app.backup.success
    {
        content = content.push(
            iced::widget::text(success.as_str())
                .size(13)
                .color(crate::style::SUCCESS),
        );
    }
    crate::ui::detail_page(
        app,
        text.backup_title,
        crate::app::Nav::Home(crate::app::Tab::Settings),
        content.into(),
    )
}
