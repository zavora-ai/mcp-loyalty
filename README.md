# Loyalty MCP Server

[![Crates.io](https://img.shields.io/crates/v/mcp-loyalty.svg)](https://crates.io/crates/mcp-loyalty)
[![License](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](LICENSE)
[![ADK-Rust Enterprise](https://img.shields.io/badge/ADK--Rust-Enterprise-purple.svg)](https://enterprise.adk-rust.com)
[![Registry Ready](https://img.shields.io/badge/ADK_Registry-Ready-green.svg)](https://www.zavora.ai)

A loyalty-program layer for [ADK-Rust Enterprise](https://enterprise.adk-rust.com) agents. 18 MCP tools over members, a points ledger, tiers, a rewards catalog, redemptions, and promotional offers — for membership, points-balance, and rewards agents.

## What It Does

The **points ledger is the source of truth**. Every balance change — earn, redeem, adjust, expire, reversal — is an append-only ledger entry carrying the running balance, so a member's balance is always reconstructable and auditable.

- **Members** with membership numbers, status, tier, current balance, and lifetime points
- **Points** — earn from spend (with tier + offer multipliers), direct awards, manual adjustments, expiry; never goes negative
- **Tiers** — Bronze → Silver → Gold → Platinum, derived from **lifetime** points (redemptions spend balance but never demote), each with an earn multiplier
- **Rewards catalog** — points cost, optional minimum tier, optional inventory
- **Redemptions** — validates tier/inventory/balance, debits via the ledger; cancelling refunds points and restores inventory
- **Offers** — promotional bonus-earn multipliers, optionally tier-targeted

## Architecture

<p align="center">
  <img src="https://raw.githubusercontent.com/zavora-ai/mcp-loyalty/main/docs/architecture.svg" alt="Loyalty MCP Architecture" width="750"/>
</p>

## Tiers

| Tier | Lifetime points | Earn multiplier |
|------|-----------------|-----------------|
| Bronze | 0 | 1.0× |
| Silver | 1,000 | 1.25× |
| Gold | 5,000 | 1.5× |
| Platinum | 20,000 | 2.0× |

## Tools (18)

### Members (5)
`enroll_member` · `get_member` · `find_member` · `set_member_status` · `get_balance` (with tier progress)

### Points (5)
| Tool | What It Does |
|------|-------------|
| `earn_points` | Earn from spend; applies tier + active-offer multipliers |
| `award_points` | Direct credit (signup bonus, goodwill) |
| `adjust_points` | Manual +/- adjustment (never below zero) |
| `expire_points` | Expire points from the balance |
| `get_ledger` | Auditable points history (running balance) |

### Rewards & Redemptions (5)
`create_reward` · `list_rewards` (tier-filtered) · `redeem_reward` · `set_redemption_status` (cancel refunds) · `get_redemptions`

### Offers (3)
`create_offer` · `list_offers` · `set_offer_active`

## Example

```
> enroll_member(name: "Test Member", contact_ref: "ref:test")   → MBR-1009 (bronze)

> earn_points(member_id: "MBR-1009", spend: 2000, reference: "order:9")
  → 4000 pts  (2000 × 1.0 tier × 2.0 "Double Points" offer) · balance 4000

> get_balance(member_id: "MBR-1009")
  → tier: silver · points_to_next_tier: 1000 → gold

> list_rewards(member_id: "MBR-1009")     ← only tier-eligible rewards
> redeem_reward(member_id: "MBR-1009", reward_id: "RWD-1006")   → RDM-1011 (pending)

> get_ledger(member_id: "MBR-1009")
  → earn  +4000 → 4000
    redeem -500 → 3500          ← running balance, always reconstructable
```

## Installation

### 1. Build

```bash
git clone https://github.com/zavora-ai/mcp-loyalty
cd mcp-loyalty
cargo build --release
```

### 2. Add to your MCP client

**Claude Desktop / Kiro / Cursor / Windsurf:**
```json
{
  "mcpServers": {
    "loyalty": {
      "command": "/path/to/mcp-loyalty"
    }
  }
}
```

### 3. Use it

```
> find_member(name: "alice")
> get_balance(member_id: "MBR-1001")
> list_rewards(member_id: "MBR-1001")
```

## Governance & Data Handling

- **Gated points writes** — `earn_points`, `award_points`, `adjust_points`, `expire_points`, `redeem_reward`, and `set_redemption_status` require approval in production (they move points of monetary value); reads are `read_only`.
- **Balance integrity** — all changes go through the ledger; the balance can never go negative, and cancellations are reversed via compensating entries.
- **No raw PII** — members carry a contact *reference*, not necessarily raw contact details.
- **Integration scaffold** — the in-memory store is for development; back it with a durable store and bind actors to authenticated identities in production.

## MCP Server Manifest

```toml
server_id = "mcp_loyalty"
display_name = "Loyalty"
version = "1.0.0"
domain = "operations"
risk_level = "medium"
writes_allowed = "gated"
transports = ["stdio"]
```

## Contributors

<!-- ALL-CONTRIBUTORS-LIST:START -->
| [<img src="https://github.com/jkmaina.png" width="80px;" alt=""/><br /><sub><b>James Karanja Maina</b></sub>](https://github.com/jkmaina) |
|:---:|
<!-- ALL-CONTRIBUTORS-LIST:END -->

## License

Apache-2.0 — see [LICENSE](LICENSE) for details.

---

Part of the [ADK-Rust Enterprise](https://enterprise.adk-rust.com) MCP server ecosystem.

## Registry Compliance

This server implements the [ADK MCP SDK](https://crates.io/crates/adk-mcp-sdk) contract:

- **HealthCheck** — async health probe for registry monitoring
- **mcp-server.toml** — manifest declaring tools, risk classes, and approval gates
- **Structured tracing** — `RUST_LOG` env-filter for observability

## rmcp and MCP compatibility

This server is built with [`rmcp` 3.1.2](https://github.com/modelcontextprotocol/rust-sdk/releases/tag/rmcp-v3.1.2) and requires Rust 1.94.1 or newer. The rmcp 3 rollout retains legacy MCP initialization compatibility and targets MCP protocol revisions `2025-11-25` and `2026-07-28`.

## MCP 2026-07-28 rollout (P4 workflow/business)

This server uses `rmcp` 3.1.2 and `adk-mcp-sdk` 0.2 with a minimum supported
Rust version of **1.94.1**. It accepts stateless MCP 2026 requests with
per-request protocol, client identity, and capability metadata while retaining
the legacy MCP 2025-11-25 initialize flow for ordinary tools.

- **Tasks:** None; this server's operations are short-lived and execute directly.
- **MRTR approvals:** `set_member_status`, `earn_points`, `award_points`, `adjust_points`, `expire_points`, `redeem_reward`, `set_redemption_status`, `set_offer_active`
- **Discovery and routing:** rmcp serves on-demand discovery and validates the
  per-request protocol envelope; HTTP deployments can route with `Mcp-Method`
  and `Mcp-Name`. The packaged binary currently uses stdio.
- **Caching:** `tools/list` returns a public `ttlMs` of 60,000 for MCP 2026;
  rmcp omits the cache fields for legacy clients.
- **Deprecated extensions:** this server does not add new Roots, Sampling, or
  dynamic client-registration dependencies.

Protected tools require `MCP_REQUEST_STATE_KEY` with at least 32 high-entropy
bytes. All replicas must share that key so sealed approval state can resume on
another instance. Approval state is bound to the client identity, tool, and
arguments and expires after two minutes. Missing identity, invalid state,
rejection, or legacy protocol use fails closed. Task records are process-local
for the current stdio runtime; use a durable task store before deploying the
server behind scale-to-zero HTTP infrastructure.
