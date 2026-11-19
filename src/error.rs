//! Errors of the SDK.

/// Result of every SDK call.
pub type Result<T> = std::result::Result<T, Error>;

/// Every error the SDK returns.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// The API answered with an error code in `response`.
    #[error(transparent)]
    Api(#[from] ApiError),

    /// Network failure, request timeout, an HTTP error without an API code, or a body that is not JSON.
    #[error("{message}")]
    Transport {
        /// What went wrong.
        message: String,
        /// HTTP status; `None` when no response arrived (network error, timeout).
        status: Option<u16>,
        /// Start of the response body (up to 500 chars), empty when none.
        body: String,
    },

    /// `wait_code` gave up: no code arrived within its `timeout`.
    #[error("{0}")]
    Timeout(String),

    /// The JSON does not have the expected shape (also: a webhook body that is not a JSON object).
    #[error("invalid response: {0}")]
    Decode(String),

    /// The client could not be built (bad base URL, TLS backend failed to start).
    #[error("invalid configuration: {0}")]
    Config(String),
}

impl Error {
    /// The API error code (`ERROR_WRONG_KEY`, `NO_NUMBER`, …) when this is [`Error::Api`].
    pub fn code(&self) -> Option<&str> {
        match self {
            Self::Api(e) => Some(&e.code),
            _ => None,
        }
    }

    /// No number is available right now (`NO_NUMBER`, `NO_NUMBER_FOR_FORWARD`).
    pub fn is_no_number(&self) -> bool {
        matches!(self, Self::Api(e) if e.is_no_number())
    }

    #[cfg(any(feature = "async", feature = "blocking"))]
    pub(crate) fn transport(message: impl Into<String>, status: Option<u16>, body: &str) -> Self {
        Self::Transport {
            message: message.into(),
            status,
            body: body.chars().take(BODY_EXCERPT).collect(),
        }
    }
}

#[cfg(any(feature = "async", feature = "blocking"))]
const BODY_EXCERPT: usize = 500;

/// An error code from the API, with a readable message.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{message} ({code})")]
pub struct ApiError {
    /// Raw error code, e.g. `ERROR_WRONG_KEY`.
    pub code: String,
    /// The server's `message`, else the [`error_message`] fallback, else the code itself.
    pub message: String,
}

impl ApiError {
    /// Error for `code`, with the server `message` when it says more than the code.
    pub fn new(code: impl Into<String>, message: Option<&str>) -> Self {
        let code = code.into();
        let message = match message {
            // Debug backends echo the code as `message`; prefer the readable fallback then.
            Some(m) if !m.is_empty() && m != code => m.to_string(),
            _ => error_message(&code).map_or_else(|| code.clone(), str::to_string),
        };
        Self { code, message }
    }

    /// `NO_NUMBER` or `NO_NUMBER_FOR_FORWARD`: retry later or pick another country.
    pub fn is_no_number(&self) -> bool {
        self.code == "NO_NUMBER" || self.code == "NO_NUMBER_FOR_FORWARD"
    }
}

/// Fallback message for an error code the server sent without a `message`.
pub fn error_message(code: &str) -> Option<&'static str> {
    Some(match code {
        "ACCOUNT_BLOCKED" => "Account is blocked",
        "ERROR_WRONG_KEY" => "Wrong API key",
        "ERROR_NO_KEY" => "API key is missing",
        "ERROR_NO_SERVICE" => "Service is not specified",
        "API_ACCESS_DISABLED" => "API access is disabled in the profile",
        "API_ACCESS_IP" => "API access from this IP is not allowed in the profile",
        "ERROR_NO_TZID" => "tzid is not specified",
        "ERROR_WRONG_TZID" => "Wrong tzid",
        "ERROR_NO_OPERATIONS" => "No operations",
        "NO_NUMBER" => "No numbers available for this service right now",
        "NO_NUMBER_FOR_FORWARD" => "No numbers available for forwarding",
        "NO_COUNTRY" => "Country not found",
        "UNDEFINED_COUNTRY" => "Unknown country",
        "WARNING_LOW_BALANCE" => "Not enough balance",
        "EXCEEDED_CONCURRENT_OPERATIONS" => "Too many concurrent operations for this account",
        "INTERVAL_CONCURRENT_REQUESTS_ERROR" => {
            "Too many concurrent requests, try again in a few seconds"
        }
        "TRY_AGAIN_LATER" => "Temporarily unable to perform the request, try again later",
        "ACCOUNT_IDENTIFICATION_REQUIRED" => "Account identification is required",
        "ERROR_PARAMS" => "Wrong parameters",
        "API_DISABLED" => "API is disabled",
        "NO_COMPLETE_TZID" => "The operation cannot be closed yet",
        "NO_CLOSE_RENT_US" => "US rent numbers cannot be closed early",
        "UNDEFINED_DAYS" => "Unknown rent period",
        "VALIDATION_ERROR" => "Validation failed",
        "TZ_OVER_OK" => "Operation finished without a new code",
        "ERROR_NO_ITEMS" => "Operation failed",
        "" => "Empty error code",
        _ => return None,
    })
}
