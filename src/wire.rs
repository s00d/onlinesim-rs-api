//! Sans-IO core shared by the async and blocking clients: request encoding and response handling.
//!
//! A section method is a [`Call`]: the request to send plus a `finish` function that turns the parsed
//! response into the result. The clients only move bytes between [`Core::http_request`], their transport
//! and [`parse_response`].

use std::fmt::Display;
use std::time::Duration;

use serde::de::DeserializeOwned;
use serde_json::{Map, Value};

use crate::error::{ApiError, Error, Result};
use crate::transport::{HttpRequest, HttpResponse, Method};

/// Default API base.
pub const DEFAULT_BASE_URL: &str = "https://onlinesim.host/api/";
/// Default per-request timeout.
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);

const USER_AGENT: &str = concat!("onlinesim-rs-api/", env!("CARGO_PKG_VERSION"));

/// Immutable client settings; built once by [`ClientBuilder`](crate::ClientBuilder).
#[derive(Debug, Clone)]
pub(crate) struct Core {
    pub(crate) api_key: Option<String>,
    pub(crate) base: reqwest::Url,
    pub(crate) lang: String,
    pub(crate) dev_id: Option<String>,
    pub(crate) timeout: Duration,
}

impl Core {
    pub(crate) fn http_request(&self, request: &Request) -> HttpRequest {
        // Endpoints are relative constants (`getNum.php`, `rent/getRentNum.php`): join cannot fail.
        let mut url = self
            .base
            .join(request.endpoint)
            .unwrap_or_else(|_| self.base.clone());
        {
            let mut pairs = url.query_pairs_mut();
            for (key, value) in &request.query {
                pairs.append_pair(key, value);
            }
            pairs.append_pair("lang", &self.lang);
            if let Some(dev_id) = &self.dev_id {
                pairs.append_pair("dev_id", dev_id);
            }
        }

        let mut headers = vec![
            ("Accept".to_string(), "application/json".to_string()),
            ("User-Agent".to_string(), USER_AGENT.to_string()),
        ];
        if let Some(key) = &self.api_key {
            headers.push(("Authorization".to_string(), format!("Bearer {key}")));
        }
        let body = request.body.as_ref().map(|b| {
            headers.push(("Content-Type".to_string(), "application/json".to_string()));
            b.to_string()
        });

        HttpRequest {
            method: request.method,
            url: url.into(),
            headers,
            body,
            timeout: self.timeout,
        }
    }
}

/// One API request before encoding. Unset and empty values are never added.
#[derive(Debug, Clone)]
pub(crate) struct Request {
    method: Method,
    endpoint: &'static str,
    query: Vec<(&'static str, String)>,
    body: Option<Value>,
}

impl Request {
    /// GET `<base><endpoint>`; `endpoint` includes its suffix (`getNum.php`, `getFreeList`).
    pub(crate) fn get(endpoint: &'static str) -> Self {
        Self {
            method: Method::Get,
            endpoint,
            query: Vec::new(),
            body: None,
        }
    }

    /// POST `<base><endpoint>` with a JSON body.
    pub(crate) fn post(endpoint: &'static str, body: Value) -> Self {
        Self {
            method: Method::Post,
            endpoint,
            query: Vec::new(),
            body: Some(body),
        }
    }

    /// Query parameter; an empty value is omitted.
    pub(crate) fn arg(mut self, key: &'static str, value: impl Display) -> Self {
        let value = value.to_string();
        if !value.is_empty() {
            self.query.push((key, value));
        }
        self
    }

    /// Boolean query parameter, sent as `1`/`0`.
    pub(crate) fn flag(self, key: &'static str, value: bool) -> Self {
        self.arg(key, if value { 1 } else { 0 })
    }

    /// Optional query parameter.
    pub(crate) fn opt(self, key: &'static str, value: Option<impl Display>) -> Self {
        match value {
            Some(v) => self.arg(key, v),
            None => self,
        }
    }

    /// Comma list (`reject=7900,7901`); an empty list is omitted.
    pub(crate) fn list(self, key: &'static str, values: &[&str]) -> Self {
        self.arg(key, values.join(","))
    }
}

/// A request plus what to make of its answer.
pub(crate) struct Call<T> {
    pub(crate) request: Request,
    pub(crate) finish: fn(Result<Value>) -> Result<T>,
}

impl<T> Call<T> {
    pub(crate) fn new(request: Request, finish: fn(Result<Value>) -> Result<T>) -> Self {
        Self { request, finish }
    }
}

/// The one response function: the JSON body is checked before the HTTP status, because a wrong key is
/// HTTP 456 with `{"response":"ERROR_WRONG_KEY"}`.
pub(crate) fn parse_response(response: HttpResponse) -> Result<Value> {
    let HttpResponse { status, body } = response;
    let data: Value = serde_json::from_str(&body).map_err(|_| {
        Error::transport(
            format!("HTTP {status}: response is not JSON"),
            Some(status),
            &body,
        )
    })?;
    if let Value::Object(mut map) = data {
        if let Some(code) = map.remove("response") {
            return unwrap(code, map);
        }
        return check_status(status, &body).map(|()| Value::Object(map));
    }
    check_status(status, &body).map(|()| data)
}

fn check_status(status: u16, body: &str) -> Result<()> {
    if (200..300).contains(&status) {
        Ok(())
    } else {
        Err(Error::transport(
            format!("HTTP {status}"),
            Some(status),
            body,
        ))
    }
}

/// Success (`1`, `"1"`, `true`): the rest of the object. Anything else: an [`ApiError`].
fn unwrap(code: Value, rest: Map<String, Value>) -> Result<Value> {
    let code = match code {
        Value::Number(n) if n.as_i64() == Some(1) => return Ok(Value::Object(rest)),
        Value::Bool(true) => return Ok(Value::Object(rest)),
        Value::String(s) if s == "1" => return Ok(Value::Object(rest)),
        // profile.php validation errors: {"response":{"webhook_url":["The webhook url must be a valid URL."]}}
        Value::Object(fields) => {
            let message = validation_messages(&fields);
            let message = (!message.is_empty()).then_some(message.as_str());
            return Err(ApiError::new("VALIDATION_ERROR", message).into());
        }
        Value::String(s) => s,
        Value::Null => String::new(),
        other => other.to_string(),
    };
    let message = rest.get("message").and_then(Value::as_str);
    Err(ApiError::new(code, message).into())
}

fn validation_messages(fields: &Map<String, Value>) -> String {
    let mut out = Vec::new();
    for value in fields.values() {
        match value {
            Value::Array(items) => out.extend(items.iter().map(text)),
            other => out.push(text(other)),
        }
    }
    out.join("; ")
}

fn text(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

/// Deserialize a parsed answer into a wire type.
pub(crate) fn decode<T: DeserializeOwned>(value: Value) -> Result<T> {
    serde_json::from_value(value).map_err(|e| Error::Decode(e.to_string()))
}
