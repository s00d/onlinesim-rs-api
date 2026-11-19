//! The HTTP boundary: one request in, status + body out.
//!
//! The default transports are [`reqwest::Client`] (async) and `reqwest::blocking::Client` (feature
//! `blocking`); pass your own (a proxy, custom TLS, a fake in tests) to
//! [`ClientBuilder::build_with`](crate::ClientBuilder::build_with) /
//! [`ClientBuilder::build_blocking_with`](crate::ClientBuilder::build_blocking_with).

use std::future::Future;
use std::pin::Pin;
use std::time::Duration;

use crate::error::{Error, Result};

/// HTTP method of a request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Method {
    /// Every endpoint except the profile update.
    Get,
    /// `profile.php` with a JSON body.
    Post,
}

/// A fully built request: the transport only has to send it.
#[derive(Debug, Clone, PartialEq)]
pub struct HttpRequest {
    /// GET or POST.
    pub method: Method,
    /// Absolute URL with the query string.
    pub url: String,
    /// Headers (`Accept`, `Authorization`, `Content-Type` for POST, `User-Agent`).
    pub headers: Vec<(String, String)>,
    /// JSON body of a POST.
    pub body: Option<String>,
    /// Per-request timeout from [`ClientBuilder::timeout`](crate::ClientBuilder::timeout).
    pub timeout: Duration,
}

/// What came back: the client parses it (body before status).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpResponse {
    /// HTTP status code.
    pub status: u16,
    /// Response body as text.
    pub body: String,
}

/// Boxed future returned by [`Transport::send`].
pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// Async transport. Failures are [`Error::Transport`] with `status: None`.
pub trait Transport: Send + Sync + 'static {
    /// Send `request`, honouring `request.timeout`.
    fn send(&self, request: HttpRequest) -> BoxFuture<'_, Result<HttpResponse>>;
}

/// Blocking transport. Failures are [`Error::Transport`] with `status: None`.
pub trait BlockingTransport: Send + Sync + 'static {
    /// Send `request`, honouring `request.timeout`.
    fn send(&self, request: HttpRequest) -> Result<HttpResponse>;
}

fn reqwest_error(error: &reqwest::Error, timeout: Duration) -> Error {
    if error.is_timeout() {
        Error::transport(format!("Request timed out after {timeout:?}"), None, "")
    } else {
        Error::transport(format!("Network error: {error}"), None, "")
    }
}

#[cfg(feature = "async")]
impl Transport for reqwest::Client {
    fn send(&self, request: HttpRequest) -> BoxFuture<'_, Result<HttpResponse>> {
        Box::pin(async move {
            let timeout = request.timeout;
            let mut builder = match request.method {
                Method::Get => self.get(&request.url),
                Method::Post => self.post(&request.url),
            }
            .timeout(timeout);
            for (name, value) in &request.headers {
                builder = builder.header(name, value);
            }
            if let Some(body) = request.body {
                builder = builder.body(body);
            }
            let response = builder
                .send()
                .await
                .map_err(|e| reqwest_error(&e, timeout))?;
            let status = response.status().as_u16();
            let body = response
                .text()
                .await
                .map_err(|e| reqwest_error(&e, timeout))?;
            Ok(HttpResponse { status, body })
        })
    }
}

#[cfg(feature = "blocking")]
impl BlockingTransport for reqwest::blocking::Client {
    fn send(&self, request: HttpRequest) -> Result<HttpResponse> {
        let timeout = request.timeout;
        let mut builder = match request.method {
            Method::Get => self.get(&request.url),
            Method::Post => self.post(&request.url),
        }
        .timeout(timeout);
        for (name, value) in &request.headers {
            builder = builder.header(name, value);
        }
        if let Some(body) = request.body {
            builder = builder.body(body);
        }
        let response = builder.send().map_err(|e| reqwest_error(&e, timeout))?;
        let status = response.status().as_u16();
        let body = response.text().map_err(|e| reqwest_error(&e, timeout))?;
        Ok(HttpResponse { status, body })
    }
}
