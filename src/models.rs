use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HttpMethod {
    Get,
    Post,
    Put,
    Patch,
    Delete,
    Head,
    Options,
}

impl HttpMethod {
    pub const ALL: [HttpMethod; 7] = [
        HttpMethod::Get,
        HttpMethod::Post,
        HttpMethod::Put,
        HttpMethod::Patch,
        HttpMethod::Delete,
        HttpMethod::Head,
        HttpMethod::Options,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            HttpMethod::Get => "GET",
            HttpMethod::Post => "POST",
            HttpMethod::Put => "PUT",
            HttpMethod::Patch => "PATCH",
            HttpMethod::Delete => "DELETE",
            HttpMethod::Head => "HEAD",
            HttpMethod::Options => "OPTIONS",
        }
    }

    pub fn next(self) -> Self {
        let idx = Self::ALL
            .iter()
            .position(|method| *method == self)
            .unwrap_or(0);
        Self::ALL[(idx + 1) % Self::ALL.len()]
    }

    pub fn previous(self) -> Self {
        let idx = Self::ALL
            .iter()
            .position(|method| *method == self)
            .unwrap_or(0);
        Self::ALL[(idx + Self::ALL.len() - 1) % Self::ALL.len()]
    }
}

impl From<HttpMethod> for reqwest::Method {
    fn from(value: HttpMethod) -> Self {
        match value {
            HttpMethod::Get => reqwest::Method::GET,
            HttpMethod::Post => reqwest::Method::POST,
            HttpMethod::Put => reqwest::Method::PUT,
            HttpMethod::Patch => reqwest::Method::PATCH,
            HttpMethod::Delete => reqwest::Method::DELETE,
            HttpMethod::Head => reqwest::Method::HEAD,
            HttpMethod::Options => reqwest::Method::OPTIONS,
        }
    }
}

impl Default for HttpMethod {
    fn default() -> Self {
        Self::Get
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Header {
    pub key: String,
    pub value: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct QueryParam {
    pub key: String,
    pub value: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuthConfig {
    None,
    Bearer { token: String },
    Basic { username: String, password: String },
    ApiKeyHeader { key: String, value: String },
}

impl Default for AuthConfig {
    fn default() -> Self {
        Self::None
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum BodyConfig {
    Empty,
    Raw { content_type: String, body: String },
}

impl Default for BodyConfig {
    fn default() -> Self {
        Self::Empty
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RequestModel {
    pub id: String,
    pub name: String,
    pub method: HttpMethod,
    pub url: String,
    pub headers: Vec<Header>,
    pub query_params: Vec<QueryParam>,
    pub auth: AuthConfig,
    pub body: BodyConfig,
}

impl Default for RequestModel {
    fn default() -> Self {
        Self {
            id: "scratch".to_string(),
            name: "Untitled request".to_string(),
            method: HttpMethod::Get,
            url: "https://example.com/api".to_string(),
            headers: vec![],
            query_params: vec![],
            auth: AuthConfig::None,
            body: BodyConfig::Empty,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ResponseModel {
    pub status: Option<u16>,
    pub status_text: String,
    pub elapsed_ms: u128,
    pub size_bytes: usize,
    pub headers: Vec<Header>,
    pub body: String,
    pub pretty_body: Option<String>,
    pub json_tree_body: Option<String>,
    pub html_body: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Collection {
    pub name: String,
    pub folders: Vec<Folder>,
    pub requests: Vec<RequestItem>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Folder {
    pub name: String,
    pub folders: Vec<Folder>,
    pub requests: Vec<RequestItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestItem {
    pub name: String,
    pub request: RequestModel,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryItem {
    pub name: String,
    pub method: HttpMethod,
    pub url: String,
    pub status: Option<u16>,
    pub elapsed_ms: u128,
    pub timestamp: String,
    pub request: RequestModel,
}
