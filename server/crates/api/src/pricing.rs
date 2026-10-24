//! Prices for a request (design §3.4): what's stored now, and what's
//! missing fetched from the price sources, in order, after the request has
//! answered. A source that fails, is rate-limited or has nothing is passed
//! over for the next; the request answers at once, with the newest stored
//! figure marked stale, and says a fetch is under way.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use domain::CommodityId;
use domain::valuation::{self, Prices, Source};
use outbound::prices::{Error, Kind, Pair, PriceSource, Quote};
use sqlx::{PgConnection, PgPool};
use store::commodities::CommodityRow;
use store::prices::PriceRow;
use time::{Date, OffsetDateTime};
use tokio::sync::watch;
use uuid::Uuid;

use crate::problem::Problem;

/// How far back of a range prices are loaded day by day; before that,
/// only each pair's latest.
const WINDOW_DAYS: i64 = 31;
/// A source that answered isn't asked for the same pair and days again
/// sooner, unless forced: a ticker it doesn't know would otherwise cost a
/// request on every page.
const COOLDOWN: Duration = Duration::from_secs(60 * 60);
/// A source that didn't answer (the network, a timeout, its server) is
/// passed over for this long, by default; one that rate-limited, for
/// `RATE_LIMITED`.
const OUTAGE: Duration = Duration::from_secs(60);
const RATE_LIMITED: Duration = Duration::from_secs(15 * 60);
/// A price for a day still in progress is fetched again when older.
const PROVISIONAL_AGE: time::Duration = time::Duration::hours(1);

/// The days each source (by name) was asked for a pair, when, and whether
/// it answered for all of the pair.
type Asked = HashMap<(String, Uuid, Uuid), Vec<(Date, Date, Instant, bool)>>;

/// Each pair's date range.
type Gaps = BTreeMap<(Uuid, Uuid), (Date, Date)>;

#[derive(Default)]
struct Memory {
    asked: Asked,
    /// Until when each source (by name) is passed over.
    down: HashMap<String, Instant>,
    /// The pairs (either way round) a fetch is under way for.
    fetching: HashSet<(Uuid, Uuid)>,
    refresh: Option<Refresh>,
}

/// The latest `POST /prices/refresh`.
#[derive(Debug, Clone)]
pub struct Refresh {
    pub started_at: OffsetDateTime,
    /// `None` while it runs.
    pub finished_at: Option<OffsetDateTime>,
    /// Prices stored or replaced.
    pub stored: usize,
    /// `source: what went wrong`, for each source that failed.
    pub errors: Vec<String>,
}

/// The price sources, in order, and how valuations choose among prices.
#[derive(Clone)]
pub struct Pricing {
    sources: Vec<Arc<dyn PriceSource>>,
    fresh_days: u32,
    outage: Duration,
    memory: Arc<Mutex<Memory>>,
    /// How many fetches are running.
    running: Arc<watch::Sender<usize>>,
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

/// The prices a request may use.
pub struct Loaded {
    pub prices: Prices,
    pub rows: Vec<PriceRow>,
    /// Prices some of these needs lack are being fetched: asked again
    /// shortly, the answer may be fresher.
    pub fetching: bool,
}

impl Pricing {
    /// `sources`: most preferred first. A price at most `fresh_days` older
    /// than the date asked is fresh.
    #[must_use]
    pub fn new(sources: Vec<Arc<dyn PriceSource>>, fresh_days: u32) -> Self {
        Self {
            sources,
            fresh_days,
            outage: OUTAGE,
            memory: Arc::default(),
            running: Arc::new(watch::Sender::new(0)),
        }
    }

    /// How long a source that didn't answer is passed over (a minute).
    #[must_use]
    pub fn outage(mut self, wait: Duration) -> Self {
        self.outage = wait;
        self
    }

    /// The sources' names, in order.
    #[must_use]
    pub fn names(&self) -> Vec<String> {
        self.sources.iter().map(|s| s.name().to_owned()).collect()
    }

    /// Waits until no fetch is running.
    pub async fn settled(&self) {
        let mut rx = self.running.subscribe();
        // The sender lives as long as `self`.
        let _ = rx.wait_for(|n| *n == 0).await;
    }

    /// The latest refresh, if any since the server started.
    #[must_use]
    pub fn last_refresh(&self) -> Option<Refresh> {
        self.lock().refresh.clone()
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

    /// The stored prices for `needs`, at once. What's missing, stale or
    /// provisional for too long is fetched in the background.
    pub async fn load(
        &self,
        pool: &PgPool,
        db: &mut PgConnection,
        today: Date,
        needs: &[Need],
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
                fetching: false,
            });
        };
        let (prices, rows) = self.stored(db, &ids, lo, hi).await?;
        let now = OffsetDateTime::now_utc();
        let gaps = ranges(needs.iter().filter(|n| {
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
        }));
        let fetching = if gaps.is_empty() || self.sources.is_empty() {
            false
        } else {
            let commodities = store::commodities::list(&mut *db, None, i64::MAX).await?;
            self.start(pool, today, commodities, gaps)
        };
        Ok(Loaded {
            prices,
            rows,
            fetching,
        })
    }

    /// Starts fetching the gaps some source would be asked for, unless
    /// they're being fetched already. Whether any are being fetched.
    fn start(
        &self,
        pool: &PgPool,
        today: Date,
        commodities: Vec<CommodityRow>,
        gaps: Gaps,
    ) -> bool {
        let mut memory = self.lock();
        let mut fetching = false;
        let mut wanted = Gaps::new();
        for (pair, days) in gaps {
            if memory.fetching.contains(&key_of(pair)) {
                fetching = true;
            } else if self.worth_asking(&mut memory, &commodities, pair, days, today) {
                wanted.insert(pair, days);
            }
        }
        if wanted.is_empty() {
            return fetching;
        }
        let guard = self.begin(&mut memory, wanted.keys().copied());
        drop(memory);
        let this = self.clone();
        let pool = pool.clone();
        tokio::spawn(async move {
            let _guard = guard;
            this.fetch(&pool, today, &commodities, &wanted, false).await;
        });
        true
    }

    /// Fetches every need's range again in the background, ignoring what
    /// was asked lately, unless a refresh is running already. The refresh
    /// that's running or just started.
    pub async fn refresh(
        &self,
        pool: &PgPool,
        db: &mut PgConnection,
        today: Date,
        needs: &[Need],
    ) -> Result<Refresh, Problem> {
        if let Some(r) = self.last_refresh().filter(|r| r.finished_at.is_none()) {
            return Ok(r);
        }
        let commodities = store::commodities::list(&mut *db, None, i64::MAX).await?;
        let gaps = ranges(needs.iter().filter(|n| n.from != n.to));
        let mut memory = self.lock();
        if let Some(r) = memory.refresh.as_ref().filter(|r| r.finished_at.is_none()) {
            return Ok(r.clone());
        }
        let started = Refresh {
            started_at: OffsetDateTime::now_utc(),
            finished_at: None,
            stored: 0,
            errors: Vec::new(),
        };
        memory.refresh = Some(started.clone());
        let guard = self.begin(&mut memory, gaps.keys().copied());
        drop(memory);
        let this = self.clone();
        let pool = pool.clone();
        tokio::spawn(async move {
            let _guard = guard;
            let (stored, errors) = this.fetch(&pool, today, &commodities, &gaps, true).await;
            if let Some(r) = this.lock().refresh.as_mut() {
                r.finished_at = Some(OffsetDateTime::now_utc());
                r.stored = stored;
                r.errors = errors;
            }
        });
        Ok(started)
    }

    /// Whether a fetch would ask some source for `pair`: one that would
    /// value it, isn't passed over, and hasn't answered for those days
    /// lately.
    fn worth_asking(
        &self,
        memory: &mut Memory,
        commodities: &[CommodityRow],
        (from, to): (Uuid, Uuid),
        days: (Date, Date),
        today: Date,
    ) -> bool {
        let (Some(base), Some(quote)) = (
            commodities.iter().find(|c| c.id == from),
            commodities.iter().find(|c| c.id == to),
        ) else {
            return false;
        };
        let Some(days) = self.window(days, today) else {
            return false;
        };
        let now = Instant::now();
        for source in &self.sources {
            if source
                .plan(&base.code, kind(base), &quote.code, kind(quote))
                .is_none()
                || memory.down.get(source.name()).is_some_and(|t| now < *t)
            {
                continue;
            }
            match asked(memory, source.name(), from, to, days) {
                Some(true) => return false,
                Some(false) => {}
                None => return true,
            }
        }
        false
    }

    /// The days to ask a source for: back far enough to cover the fresh
    /// days before the first, and no later than today.
    fn window(&self, (start, end): (Date, Date), today: Date) -> Option<(Date, Date)> {
        let start = start - time::Duration::days(i64::from(self.fresh_days));
        let end = end.min(today);
        (start <= end).then_some((start, end))
    }

    /// Marks `pairs` as being fetched until the guard is dropped.
    fn begin(&self, memory: &mut Memory, pairs: impl Iterator<Item = (Uuid, Uuid)>) -> Running {
        let pairs: Vec<_> = pairs
            .map(key_of)
            .filter(|k| memory.fetching.insert(*k))
            .collect();
        self.running.send_modify(|n| *n += 1);
        Running {
            pricing: self.clone(),
            pairs,
        }
    }

    /// Asks the sources, in order, for each gap, and stores what they
    /// answer. A source that fails is passed over for the rest of the
    /// fetch, and for a while after. `force` asks again what was asked
    /// lately, and sources passed over. How many prices were stored, and
    /// `source: what went wrong` for each source that failed.
    async fn fetch(
        &self,
        pool: &PgPool,
        today: Date,
        commodities: &[CommodityRow],
        gaps: &Gaps,
        force: bool,
    ) -> (usize, Vec<String>) {
        let mut db = match pool.acquire().await {
            Ok(db) => db,
            Err(e) => {
                tracing::error!(error = %e, "prices: no database connection");
                return (0, vec!["the database: unavailable".to_owned()]);
            }
        };
        let mut stored = 0;
        let mut errors = Vec::new();
        let mut failed: HashSet<String> = HashSet::new();
        for (&(from, to), &days) in gaps {
            let (Some(base), Some(quote)) = (
                commodities.iter().find(|c| c.id == from),
                commodities.iter().find(|c| c.id == to),
            ) else {
                continue;
            };
            let Some((start, end)) = self.window(days, today) else {
                continue;
            };
            for source in &self.sources {
                let name = source.name();
                let Some(pairs) = source.plan(&base.code, kind(base), &quote.code, kind(quote))
                else {
                    continue;
                };
                if failed.contains(name) {
                    continue;
                }
                if !force {
                    let mut memory = self.lock();
                    if memory.down.get(name).is_some_and(|t| Instant::now() < *t) {
                        continue;
                    }
                    match asked(&mut memory, name, from, to, (start, end)) {
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
                        tracing::warn!(source = name, "prices: {e}");
                        errors.push(format!("{name}: {e}"));
                        failed.insert(name.to_owned());
                        let wait = if matches!(e, Error::RateLimited) {
                            RATE_LIMITED
                        } else {
                            self.outage
                        };
                        self.lock()
                            .down
                            .insert(name.to_owned(), Instant::now() + wait);
                        continue;
                    }
                    Err(e) => {
                        tracing::error!(source = name, error = %e, "prices: the fetch panicked");
                        errors.push(format!("{name}: failed"));
                        failed.insert(name.to_owned());
                        continue;
                    }
                };
                self.lock().down.remove(name);
                let Ok((n, covered)) =
                    save(&mut db, name, commodities, &pairs, &quotes, today).await
                else {
                    errors.push("the database: storing prices failed".to_owned());
                    return (stored, errors);
                };
                stored += n;
                self.lock()
                    .asked
                    .entry(key(name, from, to))
                    .or_default()
                    .push((start, end, Instant::now(), covered));
                if covered {
                    break;
                }
            }
        }
        (stored, errors)
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Memory> {
        self.memory
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

/// A fetch under way: its pairs, until it's dropped.
struct Running {
    pricing: Pricing,
    pairs: Vec<(Uuid, Uuid)>,
}

impl Drop for Running {
    fn drop(&mut self) {
        let mut memory = self.pricing.lock();
        for p in &self.pairs {
            memory.fetching.remove(p);
        }
        drop(memory);
        self.pricing.running.send_modify(|n| *n -= 1);
    }
}

/// Whether `source` answered for all of a pair's days within the
/// cooldown, if it was asked.
fn asked(
    memory: &mut Memory,
    source: &str,
    from: Uuid,
    to: Uuid,
    days: (Date, Date),
) -> Option<bool> {
    let now = Instant::now();
    let before = memory.asked.get_mut(&key(source, from, to))?;
    before.retain(|(_, _, t, _)| now - *t < COOLDOWN);
    let (start, end) = days;
    before
        .iter()
        .rev()
        .find(|(s, e, _, _)| *s <= start && end <= *e)
        .map(|(_, _, _, answered)| *answered)
}

fn key_of((from, to): (Uuid, Uuid)) -> (Uuid, Uuid) {
    (from.min(to), from.max(to))
}

/// Either way round is the same pair.
fn key(source: &str, from: Uuid, to: Uuid) -> (String, Uuid, Uuid) {
    (source.to_owned(), from.min(to), from.max(to))
}

/// Each pair's date range.
fn ranges<'a>(needs: impl Iterator<Item = &'a Need>) -> Gaps {
    let mut out = Gaps::new();
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
/// every planned pair got at least one, either way round.
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
    // A source may quote a pair the other way round.
    let covered = pairs.iter().all(|p| {
        got.contains(&(p.base.as_str(), p.quote.as_str()))
            || got.contains(&(p.quote.as_str(), p.base.as_str()))
    });
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
