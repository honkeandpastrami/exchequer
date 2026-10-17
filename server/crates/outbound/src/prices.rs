//! Price sources (design §3.4): daily closes and reference rates, behind
//! one interface so a source can be added, replaced or dropped without
//! touching valuation. Only commodity codes and dates are sent.
//!
//! Rates are kept as the digits the source sent (`"1.0842"`): they're read
//! from the raw JSON or CSV text, never through a float.

use std::fmt;
use std::time::Duration;

use serde::Deserialize;
use serde_json::value::RawValue;
use time::Date;
use time::macros::format_description;

/// The largest response read.
const MAX_RESPONSE: u64 = 4 << 20;

/// What a commodity is, as far as a source cares.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Kind {
    Currency,
    /// A listed security or fund, quoted in USD.
    Listed,
    Other,
}

/// `1 base = rate quote`, as a source names them.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Pair {
    pub base: String,
    pub quote: String,
    pub kind: Kind,
}

/// One day's close: `1 base = rate quote`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Quote {
    pub base: String,
    pub quote: String,
    pub date: Date,
    /// Decimal digits as sent.
    pub rate: String,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Too many requests: try again later.
    #[error("rate-limited")]
    RateLimited,
    /// A bad key (401), no access (403), anything else unexpected.
    #[error("HTTP {status}{}", detail.as_deref().map(|d| format!(": {d}")).unwrap_or_default())]
    Status { status: u16, detail: Option<String> },
    /// No answer: the network, TLS, a timeout.
    #[error("unreachable: {0}")]
    Transport(String),
    /// An answer that isn't what the API documents.
    #[error("malformed response: {0}")]
    Malformed(String),
}

/// A source of prices.
pub trait PriceSource: Send + Sync {
    /// Short and lowercase (`tiingo`): prices are stored as
    /// `provider:<name>`.
    fn name(&self) -> &str;

    /// What this source would fetch to value `base` in `quote`: the pair
    /// itself, two pairs through a currency it publishes against, or `None`
    /// when it can't.
    fn plan(&self, base: &str, base_kind: Kind, quote: &str, quote_kind: Kind)
    -> Option<Vec<Pair>>;

    /// Closes for `pairs` from `from` to `to`, both included. A pair the
    /// source has no data for is left out, not an error.
    fn fetch(&self, pairs: &[Pair], from: Date, to: Date) -> Result<Vec<Quote>, Error>;
}

fn agent(timeout: Duration) -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_global(Some(timeout))
        .http_status_as_error(false)
        .build()
        .into()
}

/// `https://`, or plain HTTP on the loopback interface (tests).
fn checked_url(url: &str) -> Result<String, String> {
    let url = url.trim_end_matches('/');
    let loopback = ["http://127.0.0.1:", "http://localhost:", "http://[::1]:"]
        .iter()
        .any(|p| url.starts_with(p));
    if !url.starts_with("https://") && !loopback {
        return Err("the URL must be https://".into());
    }
    Ok(url.to_owned())
}

/// `(status, body)`.
fn get(agent: &ureq::Agent, url: &str, auth: Option<&str>) -> Result<(u16, String), Error> {
    let mut req = agent.get(url);
    if let Some(a) = auth {
        req = req.header("authorization", a);
    }
    let mut res = req.call().map_err(|e| Error::Transport(e.to_string()))?;
    let status = res.status().as_u16();
    let text = res
        .body_mut()
        .with_config()
        .limit(MAX_RESPONSE)
        .read_to_string()
        .map_err(|e| Error::Transport(e.to_string()))?;
    Ok((status, text))
}

fn iso(d: Date) -> String {
    d.format(format_description!("[year]-[month]-[day]"))
        .unwrap_or_default()
}

fn parse_date(s: &str) -> Option<Date> {
    Date::parse(s.get(..10)?, format_description!("[year]-[month]-[day]")).ok()
}

/// A JSON number's literal text, if it's a plain positive decimal.
fn digits(raw: &RawValue) -> Option<String> {
    let s = raw.get().trim();
    let ok = !s.is_empty()
        && s.chars().all(|c| c.is_ascii_digit() || c == '.')
        && s.chars().filter(|c| *c == '.').count() <= 1;
    ok.then(|| s.to_owned())
}

fn is_currency_code(code: &str) -> bool {
    code.len() == 3 && code.chars().all(|c| c.is_ascii_uppercase())
}

// --------------------------------------------------------------- Tiingo ----

/// Tiingo (api.tiingo.com): end-of-day closes for US stocks, ETFs and
/// mutual funds (in USD), and FX. Needs an API key.
pub struct Tiingo {
    agent: ureq::Agent,
    url: String,
    key: String,
}

impl fmt::Debug for Tiingo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Tiingo")
            .field("url", &self.url)
            .finish_non_exhaustive()
    }
}

impl Tiingo {
    pub const URL: &str = "https://api.tiingo.com";

    pub fn new(url: &str, key: &str, timeout: Duration) -> Result<Self, String> {
        if key.is_empty() {
            return Err("the API key is empty".into());
        }
        Ok(Self {
            agent: agent(timeout),
            url: checked_url(url)?,
            key: key.to_owned(),
        })
    }

    /// Tiingo writes share classes with a dash (`BRK-B`).
    fn ticker(code: &str) -> String {
        code.replace('.', "-").to_lowercase()
    }

    fn path(pair: &Pair) -> String {
        match pair.kind {
            Kind::Currency => format!(
                "/tiingo/fx/{}{}/prices",
                pair.base.to_lowercase(),
                pair.quote.to_lowercase()
            ),
            _ => format!("/tiingo/daily/{}/prices", Self::ticker(&pair.base)),
        }
    }
}

/// Reads a Tiingo price list (`[{"date": "…T00:00:00.000Z", "close": 1.08, …}]`).
pub fn parse_tiingo(pair: &Pair, body: &str) -> Result<Vec<Quote>, Error> {
    #[derive(Deserialize)]
    struct Row<'a> {
        date: String,
        #[serde(borrow)]
        close: Option<&'a RawValue>,
    }
    let rows: Vec<Row> = serde_json::from_str(body).map_err(|e| Error::Malformed(e.to_string()))?;
    rows.into_iter()
        .filter_map(|r| r.close.map(|c| (r.date, c)))
        .map(|(date, close)| {
            Ok(Quote {
                base: pair.base.clone(),
                quote: pair.quote.clone(),
                date: parse_date(&date).ok_or_else(|| Error::Malformed(format!("date {date}")))?,
                rate: digits(close)
                    .ok_or_else(|| Error::Malformed(format!("close {}", close.get())))?,
            })
        })
        .collect()
}

/// Tiingo's `{"detail": …}`, cut short.
fn tiingo_detail(body: &str) -> Option<String> {
    let v: serde_json::Value = serde_json::from_str(body).ok()?;
    Some(v.get("detail")?.as_str()?.chars().take(80).collect())
}

impl PriceSource for Tiingo {
    fn name(&self) -> &str {
        "tiingo"
    }

    fn plan(
        &self,
        base: &str,
        base_kind: Kind,
        quote: &str,
        quote_kind: Kind,
    ) -> Option<Vec<Pair>> {
        let pair = |base: &str, quote: &str, kind| {
            Some(vec![Pair {
                base: base.to_owned(),
                quote: quote.to_owned(),
                kind,
            }])
        };
        match (base_kind, quote_kind) {
            (Kind::Currency, Kind::Currency)
                if is_currency_code(base) && is_currency_code(quote) =>
            {
                pair(base, quote, Kind::Currency)
            }
            (Kind::Listed, Kind::Currency) if quote == "USD" => pair(base, "USD", Kind::Listed),
            (Kind::Currency, Kind::Listed) if base == "USD" => pair(quote, "USD", Kind::Listed),
            _ => None,
        }
    }

    fn fetch(&self, pairs: &[Pair], from: Date, to: Date) -> Result<Vec<Quote>, Error> {
        let auth = format!("Token {}", self.key);
        let mut out = Vec::new();
        for pair in pairs {
            let mut url = format!(
                "{}{}?startDate={}&endDate={}",
                self.url,
                Self::path(pair),
                iso(from),
                iso(to)
            );
            if pair.kind == Kind::Currency {
                url.push_str("&resampleFreq=1day");
            }
            let (status, body) = get(&self.agent, &url, Some(&auth))?;
            match status {
                200 => out.extend(parse_tiingo(pair, &body)?),
                // An unknown ticker.
                404 => {}
                429 => return Err(Error::RateLimited),
                _ => {
                    return Err(Error::Status {
                        status,
                        detail: tiingo_detail(&body),
                    });
                }
            }
        }
        Ok(out)
    }
}

// ------------------------------------------------------------------ ECB ----

/// The European Central Bank's reference rates (data-api.ecb.europa.eu):
/// about 30 currencies against the euro, each working day. Public, no key.
pub struct Ecb {
    agent: ureq::Agent,
    url: String,
}

impl fmt::Debug for Ecb {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Ecb").field("url", &self.url).finish()
    }
}

impl Ecb {
    pub const URL: &str = "https://data-api.ecb.europa.eu";

    pub fn new(url: &str, timeout: Duration) -> Result<Self, String> {
        Ok(Self {
            agent: agent(timeout),
            url: checked_url(url)?,
        })
    }
}

/// Reads the ECB's `csvdata`: one row per currency and day, `1 EUR =
/// OBS_VALUE CURRENCY`.
pub fn parse_ecb(body: &str) -> Result<Vec<Quote>, Error> {
    let mut reader = csv::Reader::from_reader(body.as_bytes());
    let headers = reader
        .headers()
        .map_err(|e| Error::Malformed(e.to_string()))?
        .clone();
    if headers.iter().all(str::is_empty) {
        return Ok(Vec::new());
    }
    let column = |name: &str| {
        headers
            .iter()
            .position(|h| h == name)
            .ok_or_else(|| Error::Malformed(format!("no {name} column")))
    };
    let (currency, denom, period, value) = (
        column("CURRENCY")?,
        column("CURRENCY_DENOM")?,
        column("TIME_PERIOD")?,
        column("OBS_VALUE")?,
    );
    let mut out = Vec::new();
    for record in reader.records() {
        let r = record.map_err(|e| Error::Malformed(e.to_string()))?;
        let field = |i: usize| r.get(i).unwrap_or_default();
        let rate = field(value);
        // A holiday may be published without a value.
        if rate.is_empty() {
            continue;
        }
        let valid = rate.chars().all(|c| c.is_ascii_digit() || c == '.');
        if !valid {
            return Err(Error::Malformed(format!("rate {rate}")));
        }
        out.push(Quote {
            base: field(denom).to_owned(),
            quote: field(currency).to_owned(),
            date: parse_date(field(period))
                .ok_or_else(|| Error::Malformed(format!("date {}", field(period))))?,
            rate: rate.to_owned(),
        });
    }
    Ok(out)
}

impl PriceSource for Ecb {
    fn name(&self) -> &str {
        "ecb"
    }

    fn plan(
        &self,
        base: &str,
        base_kind: Kind,
        quote: &str,
        quote_kind: Kind,
    ) -> Option<Vec<Pair>> {
        if base_kind != Kind::Currency
            || quote_kind != Kind::Currency
            || !is_currency_code(base)
            || !is_currency_code(quote)
        {
            return None;
        }
        let against_euro = |code: &str| Pair {
            base: "EUR".to_owned(),
            quote: code.to_owned(),
            kind: Kind::Currency,
        };
        Some(match (base, quote) {
            ("EUR", other) | (other, "EUR") => vec![against_euro(other)],
            _ => vec![against_euro(base), against_euro(quote)],
        })
    }

    fn fetch(&self, pairs: &[Pair], from: Date, to: Date) -> Result<Vec<Quote>, Error> {
        let mut currencies: Vec<&str> = pairs
            .iter()
            .filter(|p| p.base == "EUR" && is_currency_code(&p.quote))
            .map(|p| p.quote.as_str())
            .collect();
        currencies.sort_unstable();
        currencies.dedup();
        if currencies.is_empty() {
            return Ok(Vec::new());
        }
        let url = format!(
            "{}/service/data/EXR/D.{}.EUR.SP00.A?startPeriod={}&endPeriod={}&format=csvdata",
            self.url,
            currencies.join("+"),
            iso(from),
            iso(to)
        );
        let (status, body) = get(&self.agent, &url, None)?;
        match status {
            200 => parse_ecb(&body),
            // Nothing published in the range.
            404 => Ok(Vec::new()),
            429 => Err(Error::RateLimited),
            _ => Err(Error::Status {
                status,
                detail: None,
            }),
        }
    }
}

#[cfg(test)]
mod tests;
