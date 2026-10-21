//! Shared durable request-start claims. No HTTP, sleeping or credentials under SQLite.
use crate::Library;
use anyhow::{Result, ensure};
use cuerithm_autotag::{
    CachedResponse, CatalogState, MAX_CATALOG_CACHE_BYTES, MAX_CATALOG_CACHE_ENTRIES,
    MAX_CATALOG_JSON_BYTES, RateClaim, RateObservation, StartDecision,
};
use rusqlite::{OptionalExtension, params};

pub(crate) type RateProjection = (Option<i64>, Option<u32>, Option<u32>, bool);

type RateRow = (
    Option<i64>,
    i64,
    Option<u32>,
    Option<u32>,
    Option<i64>,
    bool,
    Option<i64>,
);
fn rate_row(conn: &rusqlite::Connection, service: &str) -> Result<RateRow> {
    Ok(conn.query_row("SELECT last_start_ms,not_before_ms,observed_limit,observed_remaining,observed_ms,manual_hold,manual_hold_observed_ms FROM catalog_rate_state WHERE service=?1",[service],|row|Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?,row.get(4)?,row.get(5)?,row.get(6)?))).optional()?.unwrap_or((None,0,None,None,None,false,None)))
}
fn cache_key(key: &str) -> Result<()> {
    ensure!(
        key.len() == 64 && key.bytes().all(|c| c.is_ascii_hexdigit()),
        "Catalog cache requires an opaque request digest"
    );
    Ok(())
}
impl CatalogState for Library {
    fn claim_start(&self, c: &RateClaim) -> Result<StartDecision> {
        claim_start(&mut *self.conn()?, c)
    }
    fn observe(&self, o: &RateObservation) -> Result<()> {
        observe(&mut *self.conn()?, o)
    }
    fn cache_get(&self, k: &str, now: i64) -> Result<Option<CachedResponse>> {
        cache_get(&mut *self.conn()?, k, now)
    }
    fn cache_put(&self, k: &str, v: &CachedResponse) -> Result<()> {
        cache_put(&mut *self.conn()?, k, v)
    }
}
pub(crate) fn claim_start(
    conn: &mut rusqlite::Connection,
    claim: &RateClaim,
) -> Result<StartDecision> {
    ensure!(
        claim.now_ms >= 0
            && claim.min_interval_ms <= 86_400_000
            && claim.window_ms > 0
            && claim.window_ms <= 86_400_000
            && (1..=10_000).contains(&claim.max_starts),
        "Invalid catalog request budget"
    );
    let service = claim.service.as_str();
    let window = claim.window_ms as i64;
    let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
    let (last, defer, observed_limit, remaining, observed, held, _) = rate_row(&tx, service)?;
    if held {
        tx.commit()?;
        return Ok(StartDecision::WaitUntil(i64::MAX));
    }
    tx.execute(
        "DELETE FROM catalog_request_starts WHERE service=?1 AND started_ms<=?2",
        params![service, claim.now_ms.saturating_sub(window)],
    )?;
    let starts = {
        let mut query = tx.prepare("SELECT started_ms FROM catalog_request_starts WHERE service=?1 ORDER BY started_ms,id LIMIT 10001")?;
        query
            .query_map([service], |row| row.get::<_, i64>(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?
    };
    ensure!(
        starts.len() <= 10_000,
        "Catalog request-start history exceeds its bound"
    );
    let current_observation = observed.filter(|at| at.saturating_add(window) > claim.now_ms);
    let limit = current_observation
        .and(observed_limit)
        .filter(|limit| *limit > 0)
        .map_or(claim.max_starts, |limit| limit.min(claim.max_starts));
    let mut until = defer.max(last.map_or(0, |at| at.saturating_add(claim.min_interval_ms as i64)));
    if starts.len() >= limit as usize {
        let index = starts.len() - limit as usize;
        until = until.max(starts[index].saturating_add(window));
    }
    if let (Some(at), Some(remaining)) = (current_observation, remaining) {
        let subsequent = starts.iter().filter(|start| **start > at).count();
        if subsequent >= remaining as usize {
            until = until.max(at.saturating_add(window));
        }
    }
    if until > claim.now_ms {
        tx.commit()?;
        return Ok(StartDecision::WaitUntil(until));
    }
    tx.execute(
        "INSERT INTO catalog_request_starts(service,started_ms)VALUES(?1,?2)",
        params![service, claim.now_ms],
    )?;
    tx.execute("INSERT INTO catalog_rate_state(service,last_start_ms)VALUES(?1,?2)ON CONFLICT(service)DO UPDATE SET last_start_ms=excluded.last_start_ms",params![service,claim.now_ms])?;
    tx.commit()?;
    Ok(StartDecision::Granted)
}

pub(crate) fn observe(
    conn: &mut rusqlite::Connection,
    observation: &RateObservation,
) -> Result<()> {
    ensure!(
        observation.observed_ms >= 0 && observation.not_before_ms.is_none_or(|at| at >= 0),
        "Invalid catalog response timing"
    );
    let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
    let service = observation.service.as_str();
    let (_, defer, limit, remaining, at, held, held_at) = rate_row(&tx, service)?;
    let newer = at.is_none_or(|at| observation.observed_ms >= at);
    let facts_newer = newer && (observation.limit.is_some() || observation.remaining.is_some());
    let hold_newer =
        held_at.is_none_or(|at| observation.observed_ms >= at) && observation.manual_hold.is_some();
    let next_hold = if hold_newer {
        observation.manual_hold.unwrap_or(held)
    } else {
        held
    };
    let next_held_at = if hold_newer {
        Some(observation.observed_ms)
    } else {
        held_at
    };
    tx.execute("INSERT INTO catalog_rate_state(service,not_before_ms,observed_limit,observed_remaining,observed_ms,manual_hold,manual_hold_observed_ms)VALUES(?1,?2,?3,?4,?5,?6,?7)ON CONFLICT(service)DO UPDATE SET not_before_ms=excluded.not_before_ms,observed_limit=excluded.observed_limit,observed_remaining=excluded.observed_remaining,observed_ms=excluded.observed_ms,manual_hold=excluded.manual_hold,manual_hold_observed_ms=excluded.manual_hold_observed_ms",params![service,defer.max(observation.not_before_ms.unwrap_or(0)),if facts_newer {observation.limit.or(limit)} else {limit},if facts_newer {observation.remaining} else {remaining},if facts_newer {Some(observation.observed_ms)} else {at},next_hold,next_held_at])?;
    tx.commit()?;
    Ok(())
}

pub(crate) fn cache_get(
    conn: &mut rusqlite::Connection,
    key: &str,
    now_ms: i64,
) -> Result<Option<CachedResponse>> {
    cache_key(key)?;
    ensure!(now_ms >= 0, "Invalid catalog cache time");
    let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
    tx.execute(
        "DELETE FROM catalog_response_cache WHERE expires_ms<=?1",
        [now_ms],
    )?;
    let response = tx
        .query_row(
            "SELECT body,expires_ms FROM catalog_response_cache WHERE key=?1 AND length(body)<=?2",
            params![key, MAX_CATALOG_JSON_BYTES],
            |row| {
                Ok(CachedResponse {
                    body: row.get(0)?,
                    expires_ms: row.get(1)?,
                })
            },
        )
        .optional()?;
    if response.is_some() {
        tx.execute(
            "UPDATE catalog_response_cache SET used_ms=?1 WHERE key=?2",
            params![now_ms, key],
        )?;
    }
    tx.commit()?;
    Ok(response)
}

pub(crate) fn cache_put(
    conn: &mut rusqlite::Connection,
    key: &str,
    response: &CachedResponse,
) -> Result<()> {
    cache_key(key)?;
    ensure!(
        response.body.len() <= MAX_CATALOG_JSON_BYTES && response.expires_ms >= 0,
        "Catalog cache response exceeds its bound"
    );
    let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
    tx.execute("INSERT INTO catalog_response_cache(key,body,expires_ms,used_ms)VALUES(?1,?2,?3,?4)ON CONFLICT(key)DO UPDATE SET body=excluded.body,expires_ms=excluded.expires_ms,used_ms=excluded.used_ms",params![key,response.body,response.expires_ms,crate::model::now()])?;
    loop {
        let (count, bytes): (usize, usize) = tx.query_row(
            "SELECT COUNT(*),COALESCE(SUM(length(body)),0)FROM catalog_response_cache",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;
        if count <= MAX_CATALOG_CACHE_ENTRIES && bytes <= MAX_CATALOG_CACHE_BYTES {
            break;
        }
        ensure!(tx.execute("DELETE FROM catalog_response_cache WHERE key=(SELECT key FROM catalog_response_cache ORDER BY used_ms,key LIMIT 1)",[])?==1,"Catalog cache eviction failed");
    }
    tx.commit()?;
    Ok(())
}

impl Library {
    pub(crate) fn catalog_rate_projection(&self, service: &str) -> Result<RateProjection> {
        let (_, defer, limit, remaining, _, held, _) = rate_row(&*self.conn()?, service)?;
        Ok((
            (defer > crate::model::now()).then_some(defer),
            limit,
            remaining,
            held,
        ))
    }
}

pub(crate) fn projection(conn: &rusqlite::Connection, service: &str) -> Result<RateProjection> {
    let (_, defer, limit, remaining, _, held, _) = rate_row(conn, service)?;
    Ok((
        (defer > crate::model::now()).then_some(defer),
        limit,
        remaining,
        held,
    ))
}
