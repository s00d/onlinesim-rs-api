//! Error types for the OnlineSim client.

use thiserror::Error;

/// Look up a human-readable message for an OnlineSim API error code.
pub fn request_error_message(code: &str) -> &'static str {
    match code {
        "ACCOUNT_BLOCKED" => "account blocked",
        "ERROR_WRONG_KEY" => "wrong apikey",
        "ERROR_NO_KEY" => "no apikey",
        "ERROR_NO_SERVICE" => "service not specified",
        "REQUEST_NOT_FOUND" => "API method not specified",
        "API_ACCESS_DISABLED" => "api disabled",
        "API_ACCESS_IP" => "access from this ip is disabled in the profile",
        "WARNING_NO_NUMS" => "no matching numbers",
        "TZ_INPOOL" => "waiting for a number to be dedicated to the operation",
        "TZ_NUM_WAIT" => "waiting for response",
        "TZ_NUM_ANSWER" => "response has arrived",
        "TZ_OVER_EMPTY" => "response did not arrive within the specified time",
        "TZ_OVER_OK" => "operation has been completed",
        "ERROR_NO_TZID" => "tzid is not specified",
        "ERROR_NO_OPERATIONS" => "no operations",
        "ACCOUNT_IDENTIFICATION_REQUIRED" => {
            "You have to go through an identification process: to order a messenger - in any way, for forward - on the passport."
        }
        "EXCEEDED_CONCURRENT_OPERATIONS" => {
            "maximum quantity of numbers booked concurrently is exceeded for your account"
        }
        "NO_NUMBER" => "temporarily no numbers available for the selected service",
        "TIME_INTERVAL_ERROR" => {
            "delayed SMS reception is not possible at this interval of time"
        }
        "INTERVAL_CONCURRENT_REQUESTS_ERROR" => {
            "maximum quantity of concurrent requests for number issue is exceeded, try again later"
        }
        "TRY_AGAIN_LATER" => "temporarily unable to perform the request",
        "NO_FORWARD_FOR_DEFFER" => "forwarding can be activated only for online reception",
        "NO_NUMBER_FOR_FORWARD" => "there are no numbers for forwarding",
        "ERROR_LENGTH_NUMBER_FOR_FORWARD" => "wrong length of the number for forwarding",
        "DUPLICATE_OPERATION" => "adding operations with identical parameters",
        "ERROR_NO_NUMBER" => "number is not specified",
        "ERROR_PARAMS" => "one or both parameters are wrong",
        "LIFICYCLE_NUM_EXPIRED" => "the number has expired",
        "NEED_EXTENSION_NUMBER" => "you have to extend the number, see the Extension tab",
        "ERROR_NUMBERS_PARAMS" => "error in the number format",
        "ERROR_WRONG_TZID" => "error in the number format",
        "NO_COMPLETE_TZID" => "unable to complete the operation.",
        "NO_CONFIRM_FORWARD" => "unable to confirm forwarding",
        "ERROR_NO_SERVICE_REPEAT" => "no services for repeated reception",
        "SERVICE_TO_NUMBER_EMPTY" => "no numbers for repeated reception for this service",
        _ => "unknown API error",
    }
}

/// Crate-wide result type.
pub type Result<T> = std::result::Result<T, Error>;

/// Errors returned by the client.
#[derive(Debug, Error)]
pub enum Error {
    /// No numbers available (`NO_NUMBER` / `NO_NUMBER_FOR_FORWARD`).
    #[error("no number available: {code}")]
    NoNumber {
        /// API error code.
        code: String,
    },

    /// OnlineSim API returned a non-success `response` code.
    #[error("{message} ({code})")]
    Request {
        /// API error code.
        code: String,
        /// Human-readable message when known.
        message: String,
    },

    /// `wait_code` exceeded the configured attempt limit.
    #[error("timeout waiting for SMS code")]
    Timeout,

    /// HTTP transport or status error.
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    /// JSON encode/decode failure.
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// Unexpected response shape.
    #[error("unexpected response: {0}")]
    Unexpected(String),
}

impl Error {
    /// Map an OnlineSim `response` field value to an error.
    pub fn from_api_code(code: impl Into<String>) -> Self {
        let code = code.into();
        if code == "NO_NUMBER" || code == "NO_NUMBER_FOR_FORWARD" {
            return Self::NoNumber { code };
        }
        let message = request_error_message(&code).to_string();
        Self::Request { code, message }
    }
}
