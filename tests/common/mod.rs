//! Fake transport shared by the offline tests: canned replies in order, every request recorded.
#![allow(dead_code)]

use std::collections::{BTreeMap, HashMap, VecDeque};
use std::sync::{Arc, Mutex};

use onlinesim_rs_api::{
    BlockingTransport, BoxFuture, Error, HttpRequest, HttpResponse, Method, Result, Transport,
};
use serde_json::Value;

/// A canned answer.
pub enum Reply {
    Json(u16, Value),
    Raw(u16, &'static str),
    Fail(Error),
}

/// `200` with this JSON.
pub fn ok(json: Value) -> Reply {
    Reply::Json(200, json)
}

/// A transport error as a network failure would give.
pub fn network_error(message: &str) -> Reply {
    Reply::Fail(Error::Transport {
        message: message.into(),
        status: None,
        body: String::new(),
    })
}

/// One recorded request, decoded.
#[derive(Debug, Clone)]
pub struct Call {
    pub method: Method,
    pub url: reqwest::Url,
    pub headers: HashMap<String, String>,
    pub body: Option<Value>,
    pub timeout: std::time::Duration,
}

impl Call {
    pub fn path(&self) -> &str {
        self.url.path()
    }

    /// Query without the `lang` every request carries.
    pub fn query(&self) -> BTreeMap<String, String> {
        self.url
            .query_pairs()
            .into_owned()
            .filter(|(k, _)| k != "lang")
            .collect()
    }

    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers.get(name).map(String::as_str)
    }
}

#[derive(Clone, Default)]
pub struct Fake {
    replies: Arc<Mutex<VecDeque<Reply>>>,
    calls: Arc<Mutex<Vec<Call>>>,
}

impl Fake {
    pub fn new(replies: Vec<Reply>) -> Self {
        Self {
            replies: Arc::new(Mutex::new(replies.into())),
            calls: Arc::default(),
        }
    }

    pub fn calls(&self) -> Vec<Call> {
        self.calls.lock().unwrap().clone()
    }

    /// The only request made; fails when there were more or none.
    pub fn last(&self) -> Call {
        let calls = self.calls();
        assert_eq!(calls.len(), 1, "expected 1 request, got {calls:?}");
        calls[0].clone()
    }

    fn answer(&self, request: HttpRequest) -> Result<HttpResponse> {
        let url = reqwest::Url::parse(&request.url).expect("absolute URL");
        self.calls.lock().unwrap().push(Call {
            method: request.method,
            url,
            headers: request.headers.into_iter().collect(),
            body: request
                .body
                .map(|b| serde_json::from_str(&b).expect("JSON body")),
            timeout: request.timeout,
        });
        let reply = self
            .replies
            .lock()
            .unwrap()
            .pop_front()
            .unwrap_or_else(|| panic!("unexpected request {}", request.url));
        match reply {
            Reply::Json(status, json) => Ok(HttpResponse {
                status,
                body: json.to_string(),
            }),
            Reply::Raw(status, body) => Ok(HttpResponse {
                status,
                body: body.into(),
            }),
            Reply::Fail(error) => Err(error),
        }
    }
}

impl Transport for Fake {
    fn send(&self, request: HttpRequest) -> BoxFuture<'_, Result<HttpResponse>> {
        let answer = self.answer(request);
        Box::pin(async move { answer })
    }
}

impl BlockingTransport for Fake {
    fn send(&self, request: HttpRequest) -> Result<HttpResponse> {
        self.answer(request)
    }
}

/// Async client on a fake answering `replies`.
#[cfg(feature = "async")]
pub fn client(replies: Vec<Reply>) -> (onlinesim_rs_api::Client, Fake) {
    let fake = Fake::new(replies);
    let client = onlinesim_rs_api::ClientBuilder::new()
        .api_key("test-key")
        .base_url("https://api.test/api/")
        .build_with(fake.clone())
        .unwrap();
    (client, fake)
}

/// Blocking client on a fake answering `replies`.
#[cfg(feature = "blocking")]
pub fn blocking(replies: Vec<Reply>) -> (onlinesim_rs_api::blocking::Client, Fake) {
    let fake = Fake::new(replies);
    let client = onlinesim_rs_api::ClientBuilder::new()
        .api_key("test-key")
        .base_url("https://api.test/api/")
        .build_blocking_with(fake.clone())
        .unwrap();
    (client, fake)
}

pub fn fixture(name: &str) -> Value {
    let path = format!("{}/tests/fixtures/{name}.json", env!("CARGO_MANIFEST_DIR"));
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

/// `{"a":"1"}`-style expected query.
pub fn q(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
    pairs
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect()
}
