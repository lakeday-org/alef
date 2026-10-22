mod sample_core {
    pub enum SampleError {
        Failure(String),
    }

    impl SampleError {
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
    use std::io::Write;
    for value in ["plain", "nul\0value", "control\u{1f}value", "line\nquote\"slash\\"] {
        let error = sample_error_to_napi_err(sample_core::SampleError::Failure(value.to_string()));
        writeln!(std::io::stdout(), "{}", error.cause.unwrap().reason).unwrap();
    }
}
