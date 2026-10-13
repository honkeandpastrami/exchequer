//! A decision model on Azure AI Foundry (`Microsoft-Decision-1` and the
//! like): given a `state` and named multiple-choice questions, it answers
//! each with one of the options and a probability for every option. It
//! generates no text.
//!
//! `POST <endpoint>/providers/microsoft/v1/systemone`, with the key in the
//! `api-key` header and the deployment's name as `model`. Probabilities
//! arrive as JSON numbers and are kept per mille: no float is used.

use std::collections::BTreeMap;
use std::fmt;
use std::str::FromStr;
use std::time::Duration;

use rust_decimal::prelude::ToPrimitive;
use rust_decimal::{Decimal, RoundingStrategy};
use serde::Deserialize;
use serde::ser::{SerializeMap, Serializer};

/// Attempts per request when the service is busy (429, 5xx).
const ATTEMPTS: u32 = 3;
/// The longest wait between attempts.
const MAX_WAIT: Duration = Duration::from_secs(10);
/// The largest response read.
const MAX_RESPONSE: u64 = 1 << 20;

/// A multiple-choice question: pick one of `options`, each a key and what
/// it means. Options are sent in the order given.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Question {
    pub name: String,
    pub instructions: String,
    pub options: Vec<(String, String)>,
}

/// One question's answer: the option picked and every option's probability,
/// per mille.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Answer {
    pub choice: String,
    pub probabilities: BTreeMap<String, u16>,
}

impl Answer {
    /// The picked option's probability, per mille.
    #[must_use]
    pub fn confidence(&self) -> u16 {
        self.probabilities.get(&self.choice).copied().unwrap_or(0)
    }
}

/// The answers, by question name, and the model as the service names it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Decision {
    pub model: String,
    pub answers: BTreeMap<String, Answer>,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// The service refused this request (400 or 422), e.g. a content filter
    /// or a guardrail an operator configured. Other requests may pass.
    #[error("refused ({status}{})", code.as_deref().map(|c| format!(", {c}")).unwrap_or_default())]
    Refused { status: u16, code: Option<String> },
    /// Anything else the service answered: a bad key (401), no access (403),
    /// a wrong endpoint (404), still busy after retries (429, 5xx).
    #[error("HTTP {status}{}", code.as_deref().map(|c| format!(", {c}")).unwrap_or_default())]
    Status { status: u16, code: Option<String> },
    /// No answer: the network, TLS, a timeout.
    #[error("unreachable: {0}")]
    Transport(String),
    /// An answer that isn't what the API documents.
    #[error("malformed response: {0}")]
    Malformed(String),
}

/// Anything that answers questions like a decision model: the HTTP client,
/// or a stand-in in tests.
pub trait Decide: Send + Sync {
    /// The deployment asked (its name in the service).
    fn deployment(&self) -> &str;
    fn decide(&self, state: &serde_json::Value, questions: &[Question]) -> Result<Decision, Error>;
}

/// The HTTP client.
pub struct Client {
    agent: ureq::Agent,
    url: String,
    deployment: String,
    key: String,
}

impl fmt::Debug for Client {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Client")
            .field("url", &self.url)
            .field("deployment", &self.deployment)
            .finish_non_exhaustive()
    }
}

impl Client {
    /// `endpoint`: the Foundry resource's endpoint
    /// (`https://<resource>.services.ai.azure.com`); plain HTTP only on the
    /// loopback interface, for tests.
    pub fn new(
        endpoint: &str,
        deployment: &str,
        key: &str,
        timeout: Duration,
    ) -> Result<Self, String> {
        let endpoint = endpoint.trim_end_matches('/');
        let loopback = ["http://127.0.0.1:", "http://localhost:", "http://[::1]:"]
            .iter()
            .any(|p| endpoint.starts_with(p));
        if !endpoint.starts_with("https://") && !loopback {
            return Err("the endpoint must be an https:// URL".into());
        }
        if deployment.is_empty() {
            return Err("the deployment name is empty".into());
        }
        let agent = ureq::Agent::config_builder()
            .timeout_global(Some(timeout))
            .http_status_as_error(false)
            .build()
            .into();
        Ok(Self {
            agent,
            url: format!("{endpoint}/providers/microsoft/v1/systemone"),
            deployment: deployment.to_owned(),
            key: key.to_owned(),
        })
    }

    fn body(&self, state: &serde_json::Value, questions: &[Question]) -> Result<String, Error> {
        struct Options<'a>(&'a [(String, String)]);
        impl serde::Serialize for Options<'_> {
            fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
                let mut m = s.serialize_map(Some(self.0.len()))?;
                for (k, v) in self.0 {
                    m.serialize_entry(k, v)?;
                }
                m.end()
            }
        }
        struct Questions<'a>(&'a [Question]);
        impl serde::Serialize for Questions<'_> {
            fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
                #[derive(serde::Serialize)]
                struct Q<'a> {
                    r#type: &'static str,
                    instructions: &'a str,
                    criteria: Options<'a>,
                }
                let mut m = s.serialize_map(Some(self.0.len()))?;
                for q in self.0 {
                    m.serialize_entry(
                        &q.name,
                        &Q {
                            r#type: "choice",
                            instructions: &q.instructions,
                            criteria: Options(&q.options),
                        },
                    )?;
                }
                m.end()
            }
        }
        #[derive(serde::Serialize)]
        struct Body<'a> {
            model: &'a str,
            state: &'a serde_json::Value,
            questions: Questions<'a>,
        }
        serde_json::to_string(&Body {
            model: &self.deployment,
            state,
            questions: Questions(questions),
        })
        .map_err(|e| Error::Malformed(e.to_string()))
    }
}

/// The error code in an Azure error body (`{"error": {"code": …}}`); never
/// its message, which may quote what was sent.
fn error_code(body: &str) -> Option<String> {
    let v: serde_json::Value = serde_json::from_str(body).ok()?;
    let code = v.get("error")?.get("code")?.as_str()?;
    Some(code.chars().take(60).collect())
}

/// A probability (`0.85`, `1e-05`), per mille.
fn per_mille(n: &serde_json::Number) -> Option<u16> {
    let s = n.to_string();
    let d = Decimal::from_str(&s)
        .or_else(|_| Decimal::from_scientific(&s))
        .ok()?;
    let p = (d * Decimal::ONE_THOUSAND)
        .round_dp_with_strategy(0, RoundingStrategy::MidpointAwayFromZero)
        .clamp(Decimal::ZERO, Decimal::ONE_THOUSAND);
    p.to_u16()
}

/// Reads a response body as the API documents it.
pub fn parse(body: &str) -> Result<Decision, Error> {
    #[derive(Deserialize)]
    struct RawAnswer {
        choice: Option<String>,
        probabilities: Option<BTreeMap<String, serde_json::Number>>,
    }
    #[derive(Deserialize)]
    struct Raw {
        model: Option<String>,
        answers: BTreeMap<String, RawAnswer>,
    }
    let raw: Raw = serde_json::from_str(body).map_err(|e| Error::Malformed(e.to_string()))?;
    let mut answers = BTreeMap::new();
    for (name, a) in raw.answers {
        let Some(choice) = a.choice else { continue };
        let probabilities = a
            .probabilities
            .unwrap_or_default()
            .iter()
            .map(|(k, n)| {
                per_mille(n)
                    .map(|p| (k.clone(), p))
                    .ok_or_else(|| Error::Malformed(format!("{name}: probability {n}")))
            })
            .collect::<Result<_, _>>()?;
        answers.insert(
            name,
            Answer {
                choice,
                probabilities,
            },
        );
    }
    Ok(Decision {
        model: raw.model.unwrap_or_default(),
        answers,
    })
}

impl Decide for Client {
    fn deployment(&self) -> &str {
        &self.deployment
    }

    fn decide(&self, state: &serde_json::Value, questions: &[Question]) -> Result<Decision, Error> {
        let body = self.body(state, questions)?;
        let mut wait = Duration::from_secs(1);
        for attempt in 1..=ATTEMPTS {
            let mut response = self
                .agent
                .post(&self.url)
                .header("api-key", &self.key)
                .header("content-type", "application/json")
                .send(&body)
                .map_err(|e| Error::Transport(e.to_string()))?;
            let status = response.status().as_u16();
            let retry_after = response
                .headers()
                .get("retry-after")
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.trim().parse::<u64>().ok())
                .map(Duration::from_secs);
            let text = response
                .body_mut()
                .with_config()
                .limit(MAX_RESPONSE)
                .read_to_string()
                .map_err(|e| Error::Transport(e.to_string()))?;
            match status {
                200 => return parse(&text),
                400 | 422 => {
                    return Err(Error::Refused {
                        status,
                        code: error_code(&text),
                    });
                }
                429 | 500..=599 if attempt < ATTEMPTS => {
                    std::thread::sleep(retry_after.unwrap_or(wait).min(MAX_WAIT));
                    wait *= 2;
                }
                _ => {
                    return Err(Error::Status {
                        status,
                        code: error_code(&text),
                    });
                }
            }
        }
        Err(Error::Status {
            status: 429,
            code: None,
        })
    }
}

#[cfg(test)]
mod tests;
