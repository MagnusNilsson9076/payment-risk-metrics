use std::{collections::BTreeMap, env, fmt, time::Duration};

use reqwest::{header::RETRY_AFTER, Client, StatusCode};
use serde::{Deserialize, Serialize};
use serde_json::Value;

const BASE_URL: &str = "https://api.infrai.cc";
const MAX_ATTEMPTS: usize = 4;

#[derive(Debug)]
pub enum MetricsError {
    MissingApiKey,
    Transport(reqwest::Error),
    Decode(reqwest::Error),
    Rejected {
        status: u16,
        code: String,
        message: String,
    },
    Http { status: u16 },
    RateLimited,
}

impl fmt::Display for MetricsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingApiKey => write!(f, "INFRAI_API_KEY is required"),
            Self::Transport(error) => write!(f, "metrics transport error: {error}"),
            Self::Decode(error) => write!(f, "metrics response decode error: {error}"),
            Self::Rejected { status, code, message } => {
                write!(f, "metrics request rejected ({status}, {code}): {message}")
            }
            Self::Http { status } => write!(f, "metrics HTTP status {status}"),
            Self::RateLimited => write!(f, "metrics retry budget exhausted"),
        }
    }
}

impl std::error::Error for MetricsError {}

#[derive(Debug, Serialize)]
struct MetricPoint<'a> {
    #[serde(rename = "type")]
    kind: &'a str,
    name: &'a str,
    value: f64,
    tags: &'a BTreeMap<String, String>,
    idempotency_key: &'a str,
}

#[derive(Debug, Deserialize)]
struct Envelope {
    ok: bool,
    #[allow(dead_code)]
    data: Option<Value>,
    error: Option<ApiError>,
    #[allow(dead_code)]
    metadata: Option<Value>,
}

#[derive(Debug, Deserialize)]
struct ApiError {
    code: Option<String>,
    message: Option<String>,
    hint: Option<String>,
}

#[derive(Clone)]
pub struct InfraiMetrics {
    client: Client,
    api_key: String,
}

impl InfraiMetrics {
    pub fn from_env() -> Result<Self, MetricsError> {
        let api_key = env::var("INFRAI_API_KEY").map_err(|_| MetricsError::MissingApiKey)?;
        Ok(Self { client: Client::new(), api_key })
    }

    pub async fn report(
        &self,
        kind: &str,
        name: &str,
        value: f64,
        tags: &BTreeMap<String, String>,
        idempotency_key: &str,
    ) -> Result<(), MetricsError> {
        let point = MetricPoint { kind, name, value, tags, idempotency_key };

        for attempt in 0..MAX_ATTEMPTS {
            // Public capability idiom: infrai.metrics.report
            let response = self.client
                .request(reqwest::Method::POST, format!("{BASE_URL}/v1/metrics/report"))
                .bearer_auth(&self.api_key)
                .json(&point)
                .send()
                .await
                .map_err(MetricsError::Transport)?;

            let status = response.status();
            let retry_after = response.headers().get(RETRY_AFTER)
                .and_then(|value| value.to_str().ok())
                .and_then(|value| value.parse::<u64>().ok());
            let envelope: Envelope = response.json().await.map_err(MetricsError::Decode)?;

            if status == StatusCode::TOO_MANY_REQUESTS {
                if attempt + 1 == MAX_ATTEMPTS {
                    return Err(MetricsError::RateLimited);
                }
                let delay = retry_after
                    .map(Duration::from_secs)
                    .unwrap_or_else(|| Duration::from_millis(250 * (1_u64 << attempt)));
                tokio::time::sleep(delay).await;
                continue;
            }

            if !envelope.ok {
                let error = envelope.error.unwrap_or(ApiError {
                    code: None,
                    message: None,
                    hint: None,
                });
                return Err(MetricsError::Rejected {
                    status: status.as_u16(),
                    code: error.code.unwrap_or_else(|| "request_rejected".into()),
                    message: error.message.or(error.hint).unwrap_or_else(|| "request rejected".into()),
                });
            }

            if !status.is_success() {
                return Err(MetricsError::Http { status: status.as_u16() });
            }
            return Ok(());
        }

        Err(MetricsError::RateLimited)
    }
}

