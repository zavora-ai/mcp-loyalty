//! Loyalty MCP Server library surface.
//!
//! Exposes the domain types, the in-memory loyalty store (points-ledger
//! engine), and the MCP server so integration tests can drive the same entry
//! points the JSON-RPC layer uses.

pub mod server;
pub mod store;
pub mod types;
