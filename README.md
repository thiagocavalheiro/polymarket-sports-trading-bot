# Polymarket Sports Trading Bot

**Open-source Rust bot for automated sports betting and trading on [Polymarket](https://polymarket.com).** Trade NFL, NBA, football, tennis, and any binary-outcome sports market using trailing strategies, limit orders, and hedging—with simulation and live modes. Built for **polymarket-sports-trading-bot**, **polymarket-sports-betting**, and **polymarket-sport-bet** automation.

**Repository:** [github.com/dev-protocol/polymarket-sports-trading-bot](https://github.com/dev-protocol/polymarket-sports-trading-bot)

---

## What is Polymarket?

**Polymarket** is a decentralized prediction market where you trade on real-world outcomes—elections, **sports** (NFL, NBA, soccer, tennis, etc.), and more. Prices reflect market sentiment. This **polymarket-sports-trading-bot** connects to Polymarket’s APIs (Gamma + CLOB) to automate **polymarket-sports-betting** on selected sports markets.

---

## What this bot does

| Focus | Description |
|-------|-------------|
| **Sports markets** | Trade any binary-outcome **polymarket-sport-bet** by slug (NFL, NBA, football, tennis, etc.). Trailing strategy: follow the side that moves first, then hedge the opposite. |
| **Modes** | **Simulation** (no real orders) and **live** (real orders). Backtest on historical data. |

This **polymarket-betting-bot** is built in **Rust** for speed and reliability. Use it as a **polymarket-nba-bot**, **polymarket-football-bot**, **polymarket-tennis-bot**, or for any slug-based sports market on Polymarket.

---

## Features

- **Sports trailing bot** *(default)* — Trade sports markets by Polymarket slug. Trail both outcomes; buy the side that moves first (ask ≥ lowest + trailing stop), then hedge the other. One-shot or continuous.
- **Backtest** — Replay strategy on historical price data in `history/`.
- **Test binaries** — Limit order, redeem, merge, allowance, sell, prediction tests.

---


## Quick reference

| Binary | Description |
|--------|-------------|
| `main_sports_trailing` | **Sports market trailing bot** *(default)* — **polymarket-sports-bot** by slug |
| `backtest` | Backtest on history files |
| `test_*` | test_limit_order, test_redeem, test_merge, test_allowance, test_sell, test_predict_fun |

---

## Setup

1. **Install Rust** (if needed):
   ```bash
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   ```

2. **Build:**
   ```bash
   cargo build --release
   ```

3. **Configure:** Copy `config.example.json` to `config.json` and set:
   - **polymarket:** `api_key`, `api_secret`, `api_passphrase`, `private_key`
   - Optional: `proxy_wallet_address`, `signature_type` (1 = POLY_PROXY, 2 = GNOSIS_SAFE)
   - **trading:** `slug` (market slug from Polymarket URL), `trailing_stop_point`, `trailing_shares`, `continuous`

---

## Bot versions

### 1. Sports Trailing Bot *(default)* — polymarket-sports-bot

**Binary:** `main_sports_trailing`

Trade any **binary outcome sports market** (NFL, NBA, football, tennis, etc.) by Polymarket slug. Trail both outcome tokens; buy the one whose price rises first (ask ≥ lowest + `trailing_stop_point`), then trail and buy the opposite token to hedge. Use as a **polymarket-nba-bot**, **polymarket-football-bot**, **polymarket-tennis-bot**, or for any Yes/No or Team A vs Team B market.

**Config:** Set `trading.slug` to your market slug (from the Polymarket URL). Use `continuous: true` to repeat buys until market ends, or `false` to buy each side once.

```bash
# Simulation (no real orders)
cargo run -- --simulation

# Live (real orders)
cargo run -- --no-simulation

# Explicit binary
cargo run --bin main_sports_trailing -- --config config.json --simulation
cargo run --bin main_sports_trailing -- --config config.json --no-simulation
```

### 2. Backtest

**Binary:** `backtest`

Replays the trailing strategy on `history/market_*_prices.toml`: simulated fills, hedge logic, PnL.

```bash
cargo run --bin backtest -- --backtest
```

---

## Test binaries

| Binary | Purpose |
|--------|---------|
| `test_limit_order` | Place a limit order (e.g. `--price-cents 60 --shares 10`) |
| `test_redeem` | List/redeem winning tokens (`--list`, `--redeem-all`) |
| `test_merge` | Merge complete sets to USDC (`--merge`) |
| `test_allowance` | Check balance/allowance; set approval (`--approve-only`, `--list`) |
| `test_sell` | Test market sell |
| `test_predict_fun` | Test prediction/price logic |

Example:
```bash
cargo run --bin test_allowance -- --approve-only   # One-time approval for selling
cargo run --bin test_redeem -- --list
```

---

## Configuration

- **`--simulation`** / **`--no-simulation`** — No real orders in simulation.
- **`--config <path>`** — Config file (default: `config.json`).

**Config summary:**
- **polymarket:** `gamma_api_url`, `clob_api_url`, `api_key`, `api_secret`, `api_passphrase`, `private_key`, optional `proxy_wallet_address`, `signature_type`.
- **trading:** `slug` (required), `continuous`, `trailing_stop_point` (default 0.03), `trailing_shares`, `check_interval_ms`, `min_time_remaining_seconds`.

---

## Notes

- Bots run until you stop them (Ctrl+C).
- Simulation mode logs trades but does not send orders.
- **Before selling**, set on-chain approval once per proxy wallet:  
  `cargo run --bin test_allowance -- --approve-only`

---

## Security

- Do **not** commit `config.json` with real keys or secrets.
- Prefer simulation and small sizes when testing.
- Monitor logs and balances when running in production.

---


## License & repo

**polymarket-sports-trading-bot** — [https://github.com/dev-protocol/polymarket-sports-trading-bot](https://github.com/dev-protocol/polymarket-sports-trading-bot)

**SEO keywords:** polymarket-sports-trading-bot, polymarket-sports-betting, polymarket-sports-bot, polymarket-betting-bot, polymarket-trading-bot, polymarket-nba-bot, polymarket-football-bot, polymarket-tennis-bot, polymarket-sport-bet.


---
## Search keywords

Looking for a **polymarket-sports-trading-bot**, **polymarket-sports-bot**, or **polymarket-betting-bot**? This repo is a **polymarket-trading-bot** focused on **polymarket-sports-betting**: a **polymarket-nba-bot**, **polymarket-football-bot**, **polymarket-tennis-bot**, and general **polymarket-sport-bet** automation. Keywords: _polymarket-sports-trading-bot_, _polymarket-sports-betting_, _polymarket-sports-bot_, _polymarket-betting-bot_, _polymarket-trading-bot_, _polymarket-nba-bot_, _polymarket-football-bot_, _polymarket-tennis-bot_, _polymarket-sport-bet_.

---