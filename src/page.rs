pub fn page_head(title: &str) -> String
{
    format!(
        r#"<head>
           <meta charset="UTF-8">
           <meta name="viewport" content="width=device-width, initial-scale=1.0">
           <title>{title}</title>
           <link rel="stylesheet" href="/static/style.css">
           <link rel="manifest" href="/static/manifest.json">
           <script>
               if ('serviceWorker' in navigator) {{
                   navigator.serviceWorker.register('/sw.js');
               }}
           </script>
           </head>"#,
        title = title,
    )
}
