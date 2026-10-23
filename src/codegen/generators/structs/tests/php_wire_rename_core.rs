#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub enum CallKind {
    #[serde(rename = "function")]
    Function,
}
#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct SampleCall {
    #[serde(rename = "type", alias = "callType")]
    pub call_type: CallKind,
}
