#[derive(Debug, Clone)]
pub enum ApiError
{
    Unauthorized,
    Api(String),
    Network(String),
    Decode(String),
}

impl std::fmt::Display for ApiError
{
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result
    {
        match self
        {
            ApiError::Unauthorized => write!(formatter, "未登录 / not signed in"),
            ApiError::Api(message) => write!(formatter, "{message}"),
            ApiError::Network(message) => write!(formatter, "网络错误 / network: {message}"),
            ApiError::Decode(message) => write!(formatter, "数据错误 / data: {message}"),
        }
    }
}

#[derive(Clone)]
pub struct Api
{
    base_url: String,
    token: Option<String>,
    client: reqwest::Client,
}

impl Api
{
    pub fn new(base_url: String, token: Option<String>) -> Self
    {
        #[cfg(target_arch = "wasm32")]
        let builder = reqwest::Client::builder();
        #[cfg(not(target_arch = "wasm32"))]
        let builder = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .no_proxy();
        let client = builder.build().unwrap_or_else(|_| reqwest::Client::new());
        Self {
            base_url,
            token,
            client,
        }
    }

    pub fn base_url(&self) -> &str
    {
        &self.base_url
    }

    fn url(&self, path: &str) -> String
    {
        format!("{}{}", self.base_url.trim_end_matches('/'), path)
    }

    fn prepare(&self, builder: reqwest::RequestBuilder) -> reqwest::RequestBuilder
    {
        let builder = match &self.token
        {
            Some(token) =>
            {
                builder.header(reqwest::header::AUTHORIZATION, format!("Bearer {token}"))
            },
            None => builder,
        };
        #[cfg(target_arch = "wasm32")]
        let builder = builder.fetch_credentials_include();
        builder
    }

    async fn send<T>(&self, builder: reqwest::RequestBuilder) -> Result<T, ApiError>
    where
        T: serde::de::DeserializeOwned,
    {
        let response = self
            .prepare(builder)
            .send()
            .await
            .map_err(|error| ApiError::Network(error.to_string()))?;
        let status = response.status();
        if status.is_success()
        {
            return response
                .json::<T>()
                .await
                .map_err(|error| ApiError::Decode(error.to_string()));
        }
        let body = response.text().await.unwrap_or_default();
        let message = serde_json::from_str::<serde_json::Value>(&body)
            .ok()
            .and_then(|value| {
                value
                    .get("error")
                    .and_then(|error| error.as_str())
                    .map(str::to_string)
            })
            .unwrap_or_else(|| format!("HTTP {}", status.as_u16()));
        if status.as_u16() == 401
        {
            Err(ApiError::Unauthorized)
        }
        else
        {
            Err(ApiError::Api(message))
        }
    }

    pub async fn login(
        &self,
        username: String,
        password: String,
    ) -> Result<crate::model::LoginResponse, ApiError>
    {
        let request = self
            .client
            .post(self.url("/api/v1/login"))
            .json(&serde_json::json!({
                "username": username,
                "password": password,
            }));
        self.send(request).await
    }

    pub async fn logout(&self) -> Result<serde_json::Value, ApiError>
    {
        let request = self.client.post(self.url("/api/v1/logout"));
        self.send(request).await
    }

    pub async fn me(&self) -> Result<crate::model::User, ApiError>
    {
        let request = self.client.get(self.url("/api/v1/me"));
        self.send(request).await
    }

    pub async fn update_me(&self, body_weight: Option<f64>)
    -> Result<crate::model::User, ApiError>
    {
        let request = self
            .client
            .patch(self.url("/api/v1/me"))
            .json(&crate::model::MeInput { body_weight });
        self.send(request).await
    }

    pub async fn phases(&self) -> Result<Vec<crate::model::Phase>, ApiError>
    {
        let request = self.client.get(self.url("/api/v1/phases"));
        self.send(request).await
    }

    pub async fn phase_create(
        &self,
        input: &crate::model::PhaseInput,
    ) -> Result<crate::model::Phase, ApiError>
    {
        let request = self.client.post(self.url("/api/v1/phases")).json(input);
        self.send(request).await
    }

    pub async fn phase_update(
        &self,
        id: i64,
        input: &crate::model::PhaseInput,
    ) -> Result<crate::model::Phase, ApiError>
    {
        let request = self
            .client
            .patch(self.url(&format!("/api/v1/phases/{id}")))
            .json(input);
        self.send(request).await
    }

    pub async fn phase_archive(
        &self,
        id: i64,
        archived: bool,
    ) -> Result<crate::model::Phase, ApiError>
    {
        let suffix = if archived { "archive" } else { "unarchive" };
        let request = self
            .client
            .post(self.url(&format!("/api/v1/phases/{id}/{suffix}")));
        self.send(request).await
    }

    pub async fn exercises(
        &self,
        body_part: Option<&str>,
    ) -> Result<Vec<crate::model::Exercise>, ApiError>
    {
        let mut request = self.client.get(self.url("/api/v1/exercises"));
        if let Some(body_part) = body_part.map(str::trim).filter(|part| !part.is_empty())
        {
            request = request.query(&[("body_part", body_part)]);
        }
        self.send(request).await
    }

    pub async fn exercise_create(
        &self,
        input: &crate::model::ExerciseInput,
    ) -> Result<crate::model::Exercise, ApiError>
    {
        let request = self.client.post(self.url("/api/v1/exercises")).json(input);
        self.send(request).await
    }

    pub async fn exercise_update(
        &self,
        id: i64,
        input: &crate::model::ExerciseInput,
    ) -> Result<crate::model::Exercise, ApiError>
    {
        let request = self
            .client
            .patch(self.url(&format!("/api/v1/exercises/{id}")))
            .json(input);
        self.send(request).await
    }

    pub async fn exercise_delete(&self, id: i64) -> Result<serde_json::Value, ApiError>
    {
        let request = self
            .client
            .delete(self.url(&format!("/api/v1/exercises/{id}")));
        self.send(request).await
    }

    pub async fn exercise_stats(&self, id: i64) -> Result<crate::model::ExerciseStats, ApiError>
    {
        let request = self
            .client
            .get(self.url(&format!("/api/v1/exercises/{id}/stats")));
        self.send(request).await
    }

    pub async fn templates(&self, phase_id: i64) -> Result<Vec<crate::model::Template>, ApiError>
    {
        let request = self
            .client
            .get(self.url(&format!("/api/v1/phases/{phase_id}/templates")));
        self.send(request).await
    }

    pub async fn template_create(
        &self,
        phase_id: i64,
        input: &crate::model::TemplateInput,
    ) -> Result<crate::model::Template, ApiError>
    {
        let request = self
            .client
            .post(self.url(&format!("/api/v1/phases/{phase_id}/templates")))
            .json(input);
        self.send(request).await
    }

    pub async fn template_update(
        &self,
        id: i64,
        input: &crate::model::TemplateInput,
    ) -> Result<crate::model::Template, ApiError>
    {
        let request = self
            .client
            .patch(self.url(&format!("/api/v1/templates/{id}")))
            .json(input);
        self.send(request).await
    }

    pub async fn template_delete(&self, id: i64) -> Result<serde_json::Value, ApiError>
    {
        let request = self
            .client
            .delete(self.url(&format!("/api/v1/templates/{id}")));
        self.send(request).await
    }

    pub async fn plans(
        &self,
        phase_id: i64,
        date: Option<&str>,
    ) -> Result<Vec<crate::model::Plan>, ApiError>
    {
        let mut request = self
            .client
            .get(self.url(&format!("/api/v1/phases/{phase_id}/plans")));
        if let Some(date) = date
        {
            request = request.query(&[("date", date)]);
        }
        self.send(request).await
    }

    pub async fn plan_create(
        &self,
        phase_id: i64,
        input: &crate::model::PlanInput,
    ) -> Result<crate::model::Plan, ApiError>
    {
        let request = self
            .client
            .post(self.url(&format!("/api/v1/phases/{phase_id}/plans")))
            .json(input);
        self.send(request).await
    }

    pub async fn plan_update(
        &self,
        id: i64,
        input: &crate::model::PlanInput,
    ) -> Result<crate::model::Plan, ApiError>
    {
        let request = self
            .client
            .patch(self.url(&format!("/api/v1/plans/{id}")))
            .json(input);
        self.send(request).await
    }

    pub async fn plan_delete(&self, id: i64) -> Result<serde_json::Value, ApiError>
    {
        let request = self.client.delete(self.url(&format!("/api/v1/plans/{id}")));
        self.send(request).await
    }

    pub async fn today(&self) -> Result<crate::model::Today, ApiError>
    {
        let request = self.client.get(self.url("/api/v1/today"));
        self.send(request).await
    }

    pub async fn record_upsert(
        &self,
        plan_id: i64,
        item_id: i64,
        input: &crate::model::RecordInput,
    ) -> Result<serde_json::Value, ApiError>
    {
        let request = self
            .client
            .post(self.url(&format!("/api/v1/plans/{plan_id}/items/{item_id}/records")))
            .json(input);
        self.send(request).await
    }

    pub async fn record_delete(&self, id: i64) -> Result<serde_json::Value, ApiError>
    {
        let request = self
            .client
            .delete(self.url(&format!("/api/v1/records/{id}")));
        self.send(request).await
    }

    pub async fn calendar(&self, year: i32, month: u32)
    -> Result<crate::model::Calendar, ApiError>
    {
        let request = self.client.get(self.url("/api/v1/history")).query(&[
            ("year", format!("{year:04}")),
            ("month", format!("{month:02}")),
        ]);
        self.send(request).await
    }

    pub async fn history_day(&self, date: &str) -> Result<Vec<crate::model::DayRecord>, ApiError>
    {
        let request = self
            .client
            .get(self.url(&format!("/api/v1/history/{date}")));
        self.send(request).await
    }
}
