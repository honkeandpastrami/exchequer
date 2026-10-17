//! Prices for a request (design §3.4): what's stored, and what's missing
//! fetched from the price sources, in order, inside the request. A source
//! that fails, is rate-limited or has nothing is passed over for the next,
//! and the request still answers, with the newest stored figure marked
//! stale.

use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use domain::CommodityId;
use domain::valuation::{self, Prices, Source};
use outbound::prices::{Kind, Pair, PriceSource, Quote};
use sqlx::PgConnection;
use store::commodities::CommodityRow;
use store::prices::PriceRow;
use time::{Date, OffsetDateTime};
use uuid::Uuid;

use crate::problem::Problem;

/// How far back of a range prices are loaded day by day; before that,
/// only each pair's latest.
const WINDOW_DAYS: i64 = 31;
/// A source isn't asked for the same pair and days again sooner, unless
/// forced: a ticker it doesn't know would otherwise cost a request on every
/// page.
const COOLDOWN: Duration = Duration::from_secs(60 * 60);
/// A price for a day still in progress is fetched again when older.
const PROVISIONAL_AGE: time::Duration = time::Duration::hours(1);

/// The days each source (by name) was asked for a pair, when, and whether
/// it answered for all of the pair.
type Asked = HashMap<(String, Uuid, Uuid), Vec<(Date, Date, Instant, bool)>>;

/// The price sources, in order, and how valuations choose among prices.
#[derive(Clone)]
pub struct Pricing {
    sources: Vec<Arc<dyn PriceSource>>,
    fresh_days: u32,
    asked: Arc<Mutex<Asked>>,
}

impl Default for Pricing {
    /// No sources: stored prices only.
    fn default() -> Self {
        Self::new(Vec::new(), 5)
    }
}

/// One valuation a request needs: `1 from` in `to` at the end of `date`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Need {
    pub from: Uuid,
    pub to: Uuid,
    pub date: Date,
}

/// The prices a request may use, and what fetching them did.
pub struct Loaded {
    pub prices: Prices,
    pub rows: Vec<PriceRow>,
    /// Prices stored by this request.
    pub stored: usize,
    /// `source: what went wrong`, for each source that failed.
    pub errors: Vec<String>,
}

impl Pricing {
    /// `sources`: most preferred first. A price at most `fresh_days` older
    /// than the date asked is fresh.
    #[must_use]
    pub fn new(sources: Vec<Arc<dyn PriceSource>>, fresh_days: u32) -> Self {
        Self {
            sources,
            fresh_days,
            asked: Arc::default(),
        }
    }

    /// The sources' names, in order.
    #[must_use]
    pub fn names(&self) -> Vec<String> {
        self.sources.iter().map(|s| s.name().to_owned()).collect()
    }

    /// The stored prices that may value `commodities` between two dates.
    async fn stored(
        &self,
        db: &mut PgConnection,
        commodities: &[Uuid],
        from: Date,
        to: Date,
    ) -> Result<(Prices, Vec<PriceRow>), Problem> {
        let rows =
            store::prices::for_valuation(&mut *db, commodities, from, to, WINDOW_DAYS).await?;
        let prices = rows.iter().map(to_domain).collect::<Result<Vec<_>, _>>()?;
        Ok((Prices::new(prices, self.names(), self.fresh_days), rows))
    }

    /// Prices for `needs`: those stored, after fetching what's missing,
    /// stale or provisional for too long. `force` fetches every need's
    /// range again, ignoring the cooldown.
    pub async fn load(
        &self,
        db: &mut PgConnection,
        today: Date,
        needs: &[Need],
        force: bool,
    ) -> Result<Loaded, Problem> {
        let needs: Vec<Need> = needs.iter().filter(|n| n.from != n.to).copied().collect();
        let mut ids: Vec<Uuid> = needs.iter().flat_map(|n| [n.from, n.to]).collect();
        ids.sort_unstable();
        ids.dedup();
        let (Some(lo), Some(hi)) = (
            needs.iter().map(|n| n.date).min(),
            needs.iter().map(|n| n.date).max(),
        ) else {
            return Ok(Loaded {
                prices: Prices::new(Vec::new(), self.names(), self.fresh_days),
                rows: Vec::new(),
                stored: 0,
                errors: Vec::new(),
            });
        };
        let (prices, rows) = self.stored(db, &ids, lo, hi).await?;
        let gaps = if force {
            ranges(needs.iter())
        } else {
            let now = OffsetDateTime::now_utc();
            ranges(needs.iter().filter(|n| {
                let Some(v) = prices.value(
                    CommodityId::from_uuid(n.from),
                    CommodityId::from_uuid(n.to),
                    n.date,
                ) else {
                    return true;
                };
                v.stale
                    || v.used.iter().any(|u| {
                        u.price.provisional
                            && rows
                                .iter()
                                .find(|r| same(r, &u.price))
                                .is_some_and(|r| now - r.fetched_at > PROVISIONAL_AGE)
                    })
            }))
        };
        if gaps.is_empty() || self.sources.is_empty() {
            return Ok(Loaded {
                prices,
                rows,
                stored: 0,
                errors: Vec::new(),
            });
        }
        let commodities = store::commodities::list(&mut *db, None, i64::MAX).await?;
        let mut stored = 0;
        let mut errors = Vec::new();
        for ((from, to), (start, end)) in gaps {
            let (Some(base), Some(quote)) = (
                commodities.iter().find(|c| c.id == from),
                commodities.iter().find(|c| c.id == to),
            ) else {
                continue;
            };
            let start = start - time::Duration::days(i64::from(self.fresh_days));
            let end = end.min(today);
            if start > end {
                continue;
            }
            for source in &self.sources {
                let Some(pairs) = source.plan(&base.code, kind(base), &quote.code, kind(quote))
                else {
                    continue;
                };
                let days = (start, end);
                if !force {
                    match self.asked(source.name(), from, to, days) {
                        // It answered: what it had is stored.
                        Some(true) => break,
                        Some(false) => continue,
                        None => {}
                    }
                }
                let s = Arc::clone(source);
                let wanted = pairs.clone();
                let fetched =
                    tokio::task::spawn_blocking(move || s.fetch(&wanted, start, end)).await;
                let quotes = match fetched {
                    Ok(Ok(q)) => q,
                    Ok(Err(e)) => {
                        tracing::warn!(source = source.name(), "prices: {e}");
                        errors.push(format!("{}: {e}", source.name()));
                        self.note(source.name(), from, to, days, false);
                        continue;
                    }
                    Err(_) => return Err(Problem::internal()),
                };
                let (n, covered) =
                    save(db, source.name(), &commodities, &pairs, &quotes, today).await?;
                stored += n;
                self.note(source.name(), from, to, days, covered);
                if covered {
                    break;
                }
            }
        }
        let (prices, rows) = self.stored(db, &ids, lo, hi).await?;
        Ok(Loaded {
            prices,
            rows,
            stored,
            errors,
        })
    }

    /// Whether `source` was asked for all of a pair's days within the
    /// cooldown, and if so, whether it answered.
    fn asked(&self, source: &str, from: Uuid, to: Uuid, days: (Date, Date)) -> Option<bool> {
        let mut asked = self.lock();
        let now = Instant::now();
        let before = asked.get_mut(&key(source, from, to))?;
        before.retain(|(_, _, t, _)| now - *t < COOLDOWN);
        let (start, end) = days;
        before
            .iter()
            .rev()
            .find(|(s, e, _, _)| *s <= start && end <= *e)
            .map(|(_, _, _, answered)| *answered)
    }

    fn note(&self, source: &str, from: Uuid, to: Uuid, days: (Date, Date), answered: bool) {
        self.lock().entry(key(source, from, to)).or_default().push((
            days.0,
            days.1,
            Instant::now(),
            answered,
        ));
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Asked> {
        self.asked
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

/// Either way round is the same pair.
fn key(source: &str, from: Uuid, to: Uuid) -> (String, Uuid, Uuid) {
    (source.to_owned(), from.min(to), from.max(to))
}

/// Each pair's date range.
fn ranges<'a>(needs: impl Iterator<Item = &'a Need>) -> BTreeMap<(Uuid, Uuid), (Date, Date)> {
    let mut out: BTreeMap<(Uuid, Uuid), (Date, Date)> = BTreeMap::new();
    for n in needs {
        out.entry((n.from, n.to))
            .and_modify(|(lo, hi)| {
                *lo = (*lo).min(n.date);
                *hi = (*hi).max(n.date);
            })
            .or_insert((n.date, n.date));
    }
    out
}

fn kind(c: &CommodityRow) -> Kind {
    match c.kind.as_str() {
        "fiat" => Kind::Currency,
        "security" | "fund" => Kind::Listed,
        _ => Kind::Other,
    }
}

pub fn same(r: &PriceRow, p: &valuation::Price) -> bool {
    r.base_id == *p.base.as_uuid()
        && r.quote_id == *p.quote.as_uuid()
        && r.date == p.date
        && r.source == p.source.to_string()
}

/// Stores a source's quotes for commodities that exist (a pivot such as
/// EUR needs to be a commodity). Returns how many were stored, and whether
/// every planned pair got at least one.
async fn save(
    db: &mut PgConnection,
    source: &str,
    commodities: &[CommodityRow],
    pairs: &[Pair],
    quotes: &[Quote],
    today: Date,
) -> Result<(usize, bool), Problem> {
    let id = |code: &str| commodities.iter().find(|c| c.code == code).map(|c| c.id);
    let name = Source::Provider(source.to_owned()).to_string();
    let mut stored = 0;
    let mut got: Vec<(&str, &str)> = Vec::new();
    for q in quotes {
        let (Some(base_id), Some(quote_id)) = (id(&q.base), id(&q.quote)) else {
            continue;
        };
        let Ok(rate) = q.rate.parse::<domain::Rate>() else {
            continue;
        };
        let price = valuation::Price {
            base: CommodityId::from_uuid(base_id),
            quote: CommodityId::from_uuid(quote_id),
            date: q.date,
            rate,
            source: Source::Provider(source.to_owned()),
            provisional: q.date >= today,
        };
        if price.check().is_err() {
            continue;
        }
        store::prices::store(
            &mut *db,
            &PriceRow {
                id: Uuid::now_v7(),
                base_id,
                quote_id,
                date: q.date,
                rate: price.rate.to_string(),
                source: name.clone(),
                provisional: price.provisional,
                fetched_at: OffsetDateTime::now_utc(),
                version: 1,
            },
        )
        .await?;
        stored += 1;
        got.push((&q.base, &q.quote));
    }
    let covered = pairs
        .iter()
        .all(|p| got.contains(&(p.base.as_str(), p.quote.as_str())));
    Ok((stored, covered))
}

pub fn to_domain(r: &PriceRow) -> Result<valuation::Price, Problem> {
    Ok(valuation::Price {
        base: CommodityId::from_uuid(r.base_id),
        quote: CommodityId::from_uuid(r.quote_id),
        date: r.date,
        rate: r.rate.parse().map_err(|_| Problem::internal())?,
        source: Source::parse(&r.source).ok_or_else(Problem::internal)?,
        provisional: r.provisional,
    })
}
