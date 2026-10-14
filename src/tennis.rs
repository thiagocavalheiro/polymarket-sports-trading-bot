//! Live tennis match-state data source (Live Tennis API, free tier).
//!
//! This is a *data feed only* — it never places or signs an order. It mirrors
//! the `rtds` module: a background task keeps the latest live match state in a
//! shared `Arc<Mutex<Option<TennisMatchState>>>` that the sports trailing loop
//! can read to gate or annotate its own trading decisions.
//!
//! The sports trailing bot trails Polymarket token prices with no awareness of
//! what is actually happening on court. On a tennis market that leaves two blind
//! spots the price reprices hard around: a break point (the game — and the
//! price — can swing on the next point) and a stopped match (retirement,
//! walkover, suspension) where the market is about to resolve. This module turns
//! those into an explicit, testable signal.
//!
//! Source: Live Tennis API (https://livetennisapi.com), free tier — live score,
//! who is serving, break-point flag, retirement/walkover. Base URL
//! `https://api.livetennisapi.com/api/public/v1`, auth via the `X-API-Key`
//! header. Reference (observe-only) toolkit:
//! https://github.com/livetennisapi/polymarket-tennis (MIT).
//!
//! Disclosure: this module was contributed by the maintainers of the Live
//! Tennis API. It uses only the no-card free tier; judge it on the merits.

use anyhow::{Context, Result};
use log::{debug, warn};
use serde_json::Value;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Default free-tier base URL for the Live Tennis API.
pub const DEFAULT_BASE_URL: &str = "https://api.livetennisapi.com/api/public/v1";
/// Default polling interval. The free tier allows 30 requests/minute, so 5s
/// (12 req/min) leaves ample headroom.
pub const DEFAULT_POLL_INTERVAL_SECS: u64 = 5;
const RETRY_DELAY_SECS: u64 = 5;

/// Status of a tennis match, normalized from the `status` field.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MatchStatus {
    /// Not started yet (`upcoming`/`scheduled`).
    Upcoming,
    /// In progress (`live`).
    Live,
    /// Finished normally (`completed`/`finished`).
    Completed,
    /// A player retired mid-match.
    Retired,
    /// One player did not start (walkover).
    Walkover,
    /// Temporarily halted (rain, medical timeout, etc.).
    Suspended,
    /// Any other/unknown status value, preserved verbatim.
    Other(String),
}

impl MatchStatus {
    /// Parse the API `status` string into a normalized status.
    pub fn parse(raw: &str) -> MatchStatus {
        match raw.trim().to_ascii_lowercase().as_str() {
            "live" | "inprogress" | "in_progress" => MatchStatus::Live,
            "upcoming" | "scheduled" | "pending" | "notstarted" | "not_started" => {
                MatchStatus::Upcoming
            }
            "completed" | "finished" | "ended" | "complete" => MatchStatus::Completed,
            "retired" | "retirement" => MatchStatus::Retired,
            "walkover" | "wo" | "w/o" => MatchStatus::Walkover,
            "suspended" | "interrupted" | "stopped" | "paused" | "delayed" => {
                MatchStatus::Suspended
            }
            other => MatchStatus::Other(other.to_string()),
        }
    }
}

/// Three-valued break-point flag. Break point is a receiver-side condition and
/// is only defined when the current-game points are known standard tennis
/// points. It is deliberately `Undefined` (never `Yes`) inside a tiebreak or
/// when the server / points are unknown.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BreakPoint {
    /// The receiver is one point from winning the server's game.
    Yes,
    /// Standard points are known and it is not a break point.
    No,
    /// Cannot be determined (no server, missing/non-standard points, tiebreak).
    Undefined,
}

/// A snapshot of live match state, distilled to what a trading loop can act on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TennisMatchState {
    /// Match id (the `id` field).
    pub match_id: String,
    /// Normalized match status.
    pub status: MatchStatus,
    /// Which player is serving: `Some(1)`, `Some(2)`, or `None` if unknown.
    pub server: Option<u8>,
    /// Three-valued break-point flag for the current game.
    pub break_point: BreakPoint,
}

impl TennisMatchState {
    /// True only while the match is actually being played.
    pub fn is_live(&self) -> bool {
        self.status == MatchStatus::Live
    }

    /// True when the match has stopped in a way that will resolve the market:
    /// completed, retirement, walkover, or a suspension.
    pub fn is_stopped(&self) -> bool {
        matches!(
            self.status,
            MatchStatus::Completed
                | MatchStatus::Retired
                | MatchStatus::Walkover
                | MatchStatus::Suspended
                | MatchStatus::Other(_)
        )
    }
}

/// Normalize a single point token to one of "0","15","30","40","AD", or `None`
/// if it is not a standard tennis point (e.g. a numeric tiebreak point).
fn normalize_point(token: &str) -> Option<&'static str> {
    match token.trim().to_ascii_uppercase().as_str() {
        "0" | "LOVE" => Some("0"),
        "15" => Some("15"),
        "30" => Some("30"),
        "40" => Some("40"),
        "A" | "AD" | "ADV" | "ADVANTAGE" => Some("AD"),
        _ => None,
    }
}

/// Compute the three-valued break-point flag from the server and the two
/// players' current-game point tokens.
///
/// Break point is a *receiver* condition:
/// - receiver has advantage (`AD`), or
/// - receiver has `40` while the server has `0`, `15`, or `30`.
///
/// Returns [`BreakPoint::Undefined`] when the server is unknown, a point token
/// is missing, or either token is not a standard point (which is exactly the
/// case inside a tiebreak, where break point does not apply).
pub fn break_point_flag(server: Option<u8>, p1_point: Option<&str>, p2_point: Option<&str>) -> BreakPoint {
    let server = match server {
        Some(1) | Some(2) => server.unwrap(),
        _ => return BreakPoint::Undefined,
    };
    let (Some(p1_raw), Some(p2_raw)) = (p1_point, p2_point) else {
        return BreakPoint::Undefined;
    };
    let (Some(p1), Some(p2)) = (normalize_point(p1_raw), normalize_point(p2_raw)) else {
        // Non-standard points (tiebreak) — break point is not defined.
        return BreakPoint::Undefined;
    };

    // Receiver is the player who is NOT serving.
    let (server_pt, receiver_pt) = if server == 1 { (p1, p2) } else { (p2, p1) };

    let is_break_point = receiver_pt == "AD"
        || (receiver_pt == "40" && matches!(server_pt, "0" | "15" | "30"));

    if is_break_point {
        BreakPoint::Yes
    } else {
        BreakPoint::No
    }
}

/// Read the two current-game point tokens out of a `score` object's `points`
/// value, which the API encodes as a two-element array (strings or numbers).
fn read_points(score: &Value) -> (Option<String>, Option<String>) {
    let points = match score.get("points").and_then(|p| p.as_array()) {
        Some(arr) => arr,
        None => return (None, None),
    };
    let token = |i: usize| -> Option<String> {
        points.get(i).and_then(|v| {
            if let Some(s) = v.as_str() {
                Some(s.to_string())
            } else if v.is_number() {
                Some(v.to_string())
            } else {
                None
            }
        })
    };
    (token(0), token(1))
}

/// Parse a single match object (one element of the `data` envelope) into a
/// [`TennisMatchState`]. Returns `None` if the object has no usable `id`.
pub fn parse_match_state(match_json: &Value) -> Option<TennisMatchState> {
    let match_id = match_json
        .get("id")
        .and_then(|v| {
            if let Some(s) = v.as_str() {
                Some(s.to_string())
            } else if v.is_number() {
                Some(v.to_string())
            } else {
                None
            }
        })?;

    let status = match_json
        .get("status")
        .and_then(|v| v.as_str())
        .map(MatchStatus::parse)
        .unwrap_or_else(|| MatchStatus::Other(String::new()));

    // `score` is an OBJECT: { sets, games, points, server }.
    let score = match_json.get("score");
    let server = score
        .and_then(|s| s.get("server"))
        .and_then(|v| v.as_u64())
        .and_then(|n| u8::try_from(n).ok());

    let (p1_point, p2_point) = match score {
        Some(s) => read_points(s),
        None => (None, None),
    };

    // Break point only makes sense while the match is live.
    let break_point = if status == MatchStatus::Live {
        break_point_flag(server, p1_point.as_deref(), p2_point.as_deref())
    } else {
        BreakPoint::Undefined
    };

    Some(TennisMatchState {
        match_id,
        status,
        server,
        break_point,
    })
}

/// Pull the match with `match_id` out of a `{ "data": [...] }` (or
/// `{ "data": {...} }`) response envelope and parse it.
pub fn parse_match_from_envelope(body: &Value, match_id: &str) -> Option<TennisMatchState> {
    let data = body.get("data").unwrap_or(body);
    match data {
        Value::Array(items) => items
            .iter()
            .filter_map(parse_match_state)
            .find(|m| m.match_id == match_id)
            // Fall back to the first entry when a single-match endpoint still
            // wraps the object in an array.
            .or_else(|| items.first().and_then(parse_match_state)),
        Value::Object(_) => parse_match_state(data),
        _ => None,
    }
}

/// Why the tennis overlay would hold a trade this tick.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GateReason {
    /// Match state is fine to trade on.
    Clear,
    /// A break point is in play — the price is about to move sharply.
    BreakPoint,
    /// The match has stopped (completed / retirement / walkover / suspension).
    MatchStopped,
    /// No state available yet (fail-open: the overlay never freezes the bot).
    NoState,
}

/// Decision returned to the trading loop.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TradeGate {
    /// Whether the loop may act on price this tick.
    pub allow_trade: bool,
    /// The reason for the decision (for logging / annotation).
    pub reason: GateReason,
}

/// Decide whether the trailing loop should act this tick, given the latest
/// tennis match state.
///
/// The overlay is intentionally conservative and *fail-open*: with no state
/// (`None`) it allows the trade so an upstream data hiccup can never strand the
/// bot. It holds a trade only when there is a concrete reason — a break point
/// in play, or a match that has stopped and is about to resolve.
pub fn evaluate_gate(state: Option<&TennisMatchState>) -> TradeGate {
    let Some(state) = state else {
        return TradeGate {
            allow_trade: true,
            reason: GateReason::NoState,
        };
    };

    if state.is_stopped() {
        return TradeGate {
            allow_trade: false,
            reason: GateReason::MatchStopped,
        };
    }

    if state.is_live() && state.break_point == BreakPoint::Yes {
        return TradeGate {
            allow_trade: false,
            reason: GateReason::BreakPoint,
        };
    }

    TradeGate {
        allow_trade: true,
        reason: GateReason::Clear,
    }
}

/// Fetch the current match state once from the Live Tennis API.
async fn fetch_match_state(
    client: &reqwest::Client,
    base_url: &str,
    api_key: &str,
    match_id: &str,
) -> Result<Option<TennisMatchState>> {
    let url = format!(
        "{}/matches/{}",
        base_url.trim_end_matches('/'),
        match_id
    );
    let resp = client
        .get(&url)
        .header("X-API-Key", api_key)
        .send()
        .await
        .context("Live Tennis API request failed")?;
    let status = resp.status();
    let body: Value = resp
        .json()
        .await
        .context("Live Tennis API returned non-JSON body")?;
    if !status.is_success() {
        anyhow::bail!("Live Tennis API returned HTTP {}", status);
    }
    Ok(parse_match_from_envelope(&body, match_id))
}

/// Spawn a background task that keeps the latest [`TennisMatchState`] for
/// `match_id` in `out`. Polls every `poll_interval_secs` and retries on error,
/// mirroring the `rtds` module. The task never trades — it only publishes state.
pub fn spawn_tennis_state_task(
    client: reqwest::Client,
    base_url: String,
    api_key: String,
    match_id: String,
    poll_interval_secs: u64,
    out: Arc<Mutex<Option<TennisMatchState>>>,
) {
    let interval = poll_interval_secs.max(1);
    tokio::spawn(async move {
        loop {
            match fetch_match_state(&client, &base_url, &api_key, &match_id).await {
                Ok(Some(state)) => {
                    debug!(
                        "Tennis match {} state: {:?} server={:?} break_point={:?}",
                        match_id, state.status, state.server, state.break_point
                    );
                    *out.lock().await = Some(state);
                    tokio::time::sleep(tokio::time::Duration::from_secs(interval)).await;
                }
                Ok(None) => {
                    debug!("Tennis match {} not found in feed", match_id);
                    tokio::time::sleep(tokio::time::Duration::from_secs(interval)).await;
                }
                Err(e) => {
                    warn!(
                        "Tennis state poll error for {}: {} - retrying in {}s",
                        match_id, e, RETRY_DELAY_SECS
                    );
                    tokio::time::sleep(tokio::time::Duration::from_secs(RETRY_DELAY_SECS)).await;
                }
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn status_parsing_is_normalized() {
        assert_eq!(MatchStatus::parse("live"), MatchStatus::Live);
        assert_eq!(MatchStatus::parse("LIVE"), MatchStatus::Live);
        assert_eq!(MatchStatus::parse("upcoming"), MatchStatus::Upcoming);
        assert_eq!(MatchStatus::parse("completed"), MatchStatus::Completed);
        assert_eq!(MatchStatus::parse("retired"), MatchStatus::Retired);
        assert_eq!(MatchStatus::parse("walkover"), MatchStatus::Walkover);
        assert_eq!(MatchStatus::parse("suspended"), MatchStatus::Suspended);
        assert_eq!(
            MatchStatus::parse("mystery"),
            MatchStatus::Other("mystery".to_string())
        );
    }

    #[test]
    fn break_point_receiver_advantage() {
        // Server 1, receiver (p2) has AD -> break point.
        assert_eq!(break_point_flag(Some(1), Some("40"), Some("AD")), BreakPoint::Yes);
        // Server 2, receiver (p1) has AD -> break point.
        assert_eq!(break_point_flag(Some(2), Some("AD"), Some("40")), BreakPoint::Yes);
    }

    #[test]
    fn break_point_receiver_40_vs_server_low() {
        // Server 1 at 30, receiver (p2) at 40 -> break point.
        assert_eq!(break_point_flag(Some(1), Some("30"), Some("40")), BreakPoint::Yes);
        assert_eq!(break_point_flag(Some(1), Some("0"), Some("40")), BreakPoint::Yes);
        assert_eq!(break_point_flag(Some(1), Some("15"), Some("40")), BreakPoint::Yes);
    }

    #[test]
    fn not_break_point_when_server_ahead_or_deuce() {
        // Deuce (40-40) is not a break point.
        assert_eq!(break_point_flag(Some(1), Some("40"), Some("40")), BreakPoint::No);
        // Server has advantage (game point for server), not a break point.
        assert_eq!(break_point_flag(Some(1), Some("AD"), Some("40")), BreakPoint::No);
        // Server at 40, receiver at 30 -> server game point, not a break point.
        assert_eq!(break_point_flag(Some(1), Some("40"), Some("30")), BreakPoint::No);
    }

    #[test]
    fn break_point_undefined_without_server_or_points() {
        assert_eq!(break_point_flag(None, Some("40"), Some("AD")), BreakPoint::Undefined);
        assert_eq!(break_point_flag(Some(1), None, Some("40")), BreakPoint::Undefined);
        // Server value out of range.
        assert_eq!(break_point_flag(Some(3), Some("40"), Some("AD")), BreakPoint::Undefined);
    }

    #[test]
    fn break_point_undefined_in_tiebreak() {
        // Tiebreak points are numeric (6-7), not standard tennis points.
        assert_eq!(break_point_flag(Some(1), Some("6"), Some("7")), BreakPoint::Undefined);
        assert_eq!(break_point_flag(Some(2), Some("7"), Some("8")), BreakPoint::Undefined);
    }

    #[test]
    fn parse_live_match_with_break_point() {
        let body = json!({
            "id": "m123",
            "status": "live",
            "players": { "p1": { "name": "A" }, "p2": { "name": "B" } },
            "score": { "sets": [], "games": [[4, 5]], "points": ["30", "40"], "server": 1 }
        });
        let state = parse_match_state(&body).unwrap();
        assert_eq!(state.match_id, "m123");
        assert_eq!(state.status, MatchStatus::Live);
        assert_eq!(state.server, Some(1));
        assert_eq!(state.break_point, BreakPoint::Yes);
    }

    #[test]
    fn parse_ignores_break_point_when_not_live() {
        let body = json!({
            "id": "m999",
            "status": "completed",
            "score": { "points": ["30", "40"], "server": 1 }
        });
        let state = parse_match_state(&body).unwrap();
        assert_eq!(state.status, MatchStatus::Completed);
        // Not live -> break point is not asserted.
        assert_eq!(state.break_point, BreakPoint::Undefined);
    }

    #[test]
    fn parse_from_envelope_finds_by_id() {
        let body = json!({
            "data": [
                { "id": "a", "status": "live", "score": { "points": ["0", "0"], "server": 1 } },
                { "id": "b", "status": "live", "score": { "points": ["40", "30"], "server": 2 } }
            ]
        });
        let state = parse_match_from_envelope(&body, "b").unwrap();
        assert_eq!(state.match_id, "b");
        assert_eq!(state.server, Some(2));
        // Server is p2 at 30; receiver p1 at 40 -> break point on the receiver.
        assert_eq!(state.break_point, BreakPoint::Yes);
    }

    #[test]
    fn gate_holds_on_break_point() {
        let state = TennisMatchState {
            match_id: "m".into(),
            status: MatchStatus::Live,
            server: Some(1),
            break_point: BreakPoint::Yes,
        };
        let gate = evaluate_gate(Some(&state));
        assert!(!gate.allow_trade);
        assert_eq!(gate.reason, GateReason::BreakPoint);
    }

    #[test]
    fn gate_holds_on_stopped_match() {
        for status in [MatchStatus::Completed, MatchStatus::Retired, MatchStatus::Walkover, MatchStatus::Suspended] {
            let state = TennisMatchState {
                match_id: "m".into(),
                status,
                server: None,
                break_point: BreakPoint::Undefined,
            };
            let gate = evaluate_gate(Some(&state));
            assert!(!gate.allow_trade);
            assert_eq!(gate.reason, GateReason::MatchStopped);
        }
    }

    #[test]
    fn gate_allows_clear_live_state() {
        let state = TennisMatchState {
            match_id: "m".into(),
            status: MatchStatus::Live,
            server: Some(1),
            break_point: BreakPoint::No,
        };
        let gate = evaluate_gate(Some(&state));
        assert!(gate.allow_trade);
        assert_eq!(gate.reason, GateReason::Clear);
    }

    #[test]
    fn gate_fails_open_without_state() {
        let gate = evaluate_gate(None);
        assert!(gate.allow_trade);
        assert_eq!(gate.reason, GateReason::NoState);
    }
}
