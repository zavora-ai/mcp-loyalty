# Changelog

## [1.1.0] - 2026-08-13

### Changed
- Upgraded to rmcp 3.1.2 and raised the minimum supported Rust version to 1.94.1.
- Added MCP 2026-07-28 stateless request handling while retaining MCP 2025-11-25 initialization compatibility.

### Added
- Per-request identity and protocol metadata, on-demand discovery/cache hints, and the configured Tasks and sealed MRTR approval policies.

## [1.0.0] - 2026-06-09

### Added
- 18 MCP tools: 5 member + 5 points + 5 rewards/redemptions + 3 offers
- Append-only **points ledger** as the source of truth for balances (earn/redeem/adjust/expire/reversal),
  each entry carrying the running balance for full auditability
- Tiers (Bronze/Silver/Gold/Platinum) derived from lifetime points, with per-tier earn multipliers;
  redemptions spend balance but never demote a member
- Earn from spend with tier + active-offer multipliers; direct awards; manual adjustments; expiry
- Balance integrity: never goes negative; suspended/closed accounts cannot be credited or redeem
- Rewards catalog with points cost, optional minimum tier, and optional inventory
- Redemptions with tier/inventory/balance validation; cancellation refunds points and restores inventory
- Promotional offers with bonus-earn multipliers and optional tier targeting
- Gated points-mutating writes (earn/award/adjust/expire/redeem/redemption-status)
- `adk-mcp-sdk` HealthCheck + validated `mcp-server.toml` manifest
- 14 tests (11 store + 3 manifest); verified end-to-end over MCP stdio
