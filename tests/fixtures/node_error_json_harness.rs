mod sample_core {
    pub enum SampleError {
        Failure(String),
    }

    impl SampleError {
        pub fn status_code(&self) -> u16 {
            429
        }

        pub fn is_transient(&self) -> bool {
            true
        }

        pub fn retry_after(&self) -> Option<std::time::Duration> {
            match self.error_type() {
                "plain" => None,
                "control\u{1f}value" => Some(std::time::Duration::MAX),
                _ => Some(std::time::Duration::from_secs(5)),
            }
        }

        pub fn timeout(&self) -> std::time::Duration {
            if self.error_type() == "control\u{1f}value" {
                std::time::Duration::MAX
            } else {
                std::time::Duration::from_millis(1250)
            }
        }

        pub fn error_type(&self) -> &str {
            match self {
                Self::Failure(value) => value,
            }
        }
    }

    impl std::fmt::Display for SampleError {
        fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter.write_str("operation failed")
        }
    }
}

mod napi {
    pub enum Status {
        GenericFailure,
    }

    pub struct Error {
        pub reason: String,
        pub cause: Option<Box<Self>>,
    }

    impl Error {
        pub fn new(_: Status, reason: String) -> Self {
            Self { reason, cause: None }
        }

        pub fn set_cause(&mut self, cause: Self) {
            self.cause = Some(Box::new(cause));
        }
    }
}

/* CONVERTER */

fn main() {
    for value in ["plain", "nul\0value", "control\u{1f}value", "line\nquote\"slash\\"] {
        let error = sample_error_to_napi_err(sample_core::SampleError::Failure(value.to_string()));
        println!("{}", error.cause.unwrap().reason);
    }
}
