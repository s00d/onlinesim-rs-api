//! The optional HTTP front of the mock (feature `mock`): a local `wiremock` server whose one responder
//! hands every `/api/` request to the same dispatcher as the in-process transport.

use wiremock::{Mock, MockServer, Request, Respond, ResponseTemplate};

use super::{MockRequest, Shared, api_path};
use crate::transport::Method;

pub(super) struct Server(MockServer);

impl Server {
    pub(super) async fn start(shared: Shared) -> Self {
        let server = MockServer::start().await;
        Mock::given(wiremock::matchers::path_regex(r"^/api/"))
            .respond_with(Responder(shared))
            .mount(&server)
            .await;
        Self(server)
    }

    pub(super) fn base_url(&self) -> String {
        format!("{}/api/", self.0.uri())
    }
}

struct Responder(Shared);

impl Respond for Responder {
    fn respond(&self, request: &Request) -> ResponseTemplate {
        let method = if request.method.as_str().eq_ignore_ascii_case("POST") {
            Method::Post
        } else {
            Method::Get
        };
        let body = std::str::from_utf8(&request.body).ok();
        let mock_request = MockRequest::new(
            method,
            api_path(request.url.path()),
            request.url.query_pairs().into_owned().collect(),
            body.filter(|b| !b.is_empty()),
        );
        let authorization = request
            .headers
            .get("authorization")
            .and_then(|v| v.to_str().ok());
        let response = self.0.dispatch(mock_request, authorization);
        let content_type = if response.body.starts_with('<') {
            "text/html"
        } else {
            "application/json"
        };
        ResponseTemplate::new(response.status).set_body_raw(response.body, content_type)
    }
}
