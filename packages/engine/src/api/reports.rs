use crate::State;
use crate::util::fetch::fetch_json;
use reqwest::Method;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ReportItemType {
    Project,
    Version,
    User,
    SharedInstance,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreateReportRequest {
    pub report_type: String,
    pub item_id: String,
    pub item_type: ReportItemType,
    pub body: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub uploaded_images: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreateReportResponse {
    pub id: String,
}

#[tracing::instrument(skip(request))]
pub async fn create_report(
    request: CreateReportRequest,
) -> crate::Result<CreateReportResponse> {
    if crate::self_hosted::accounts::enabled()
        && matches!(
            request.item_type,
            ReportItemType::User | ReportItemType::SharedInstance
        )
    {
        return Err(crate::ErrorKind::InputError(
            "Reporting local accounts and shared instances is not supported by this service yet."
                .to_string(),
        )
        .into());
    }
    let state = State::get().await?;

    fetch_json(
        Method::POST,
        &format!("{}report", env!("MODRINTH_API_URL_V3")),
        None,
        Some(serde_json::to_value(request)?),
        Some("/v3/report"),
        &state.api_semaphore,
        &state.pool,
    )
    .await
}
