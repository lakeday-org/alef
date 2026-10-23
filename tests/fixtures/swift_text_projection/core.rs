#[derive(Clone, serde::Serialize, serde::Deserialize)]
#[serde(untagged)]
pub enum BodyValue {
    PlainValue(String),
    Count(i32),
}

impl std::fmt::Display for BodyValue {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::PlainValue(value) => formatter.write_str(value),
            Self::Count(value) => write!(formatter, "{value}"),
        }
    }
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct Packet {
    pub required: BodyValue,
    pub nullable: Option<BodyValue>,
    pub legacy: Option<BodyValue>,
}
