use serde::Serialize;

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivatedPayload {
    pub tab_id: Option<String>,
}
