#[derive(Debug, thiserror::Error)]
pub enum SampleError {
    #[error("not_found: {message}")]
    Missing { message: String, status: u16 },
    #[error("policy_rejected: {url} - {reason}")]
    Policy { url: String, reason: String },
    #[error("numbers: {count}/{enabled}")]
    Numbers { count: u32, enabled: bool },
    #[error("escaped {{ {message} }} $quote \" slash \\ newline\n")]
    Escaped { message: String },
    #[error("unavailable")]
    Unit,
    #[error("keyword: {required}")]
    Keyword { required: String },
    #[error("debug: {message:?}")]
    Debug { message: String },
}

#[derive(Debug, thiserror::Error)]
pub enum SimpleError {
    #[error("native-only")]
    Only,
}

pub fn fail_case(mode: u8) -> Result<(), SampleError> {
    Err(match mode {
        0 => SampleError::Missing {
            message: "missing payload".into(),
            status: 404,
        },
        1 => SampleError::Policy {
            url: "https://example.test/".into(),
            reason: "denied".into(),
        },
        2 => SampleError::Numbers {
            count: 7,
            enabled: true,
        },
        3 => SampleError::Escaped {
            message: "raw $ {value}".into(),
        },
        4 => SampleError::Unit,
        5 => SampleError::Keyword {
            required: "required raw".into(),
        },
        _ => SampleError::Debug {
            message: "debug raw".into(),
        },
    })
}
