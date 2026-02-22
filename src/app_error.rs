use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExitCode {
    Success = 0,
    RuntimeFailure = 1,
    Usage = 2,
    Internal = 3,
}

#[derive(Debug)]
pub struct AppError {
    code: ExitCode,
    message: String,
}

impl AppError {
    pub fn usage<T: Into<String>>(message: T) -> Self {
        Self {
            code: ExitCode::Usage,
            message: message.into(),
        }
    }

    pub fn runtime<T: Into<String>>(message: T) -> Self {
        Self {
            code: ExitCode::RuntimeFailure,
            message: message.into(),
        }
    }

    pub fn internal<T: Into<String>>(message: T) -> Self {
        Self {
            code: ExitCode::Internal,
            message: message.into(),
        }
    }

    pub fn code(&self) -> i32 {
        self.code as i32
    }
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for AppError {}
