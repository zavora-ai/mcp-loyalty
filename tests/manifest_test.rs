//! Validate mcp-server.toml parses, passes SDK validation, has the right tool
//! count, and gates the points-mutating writes.

use adk_mcp_sdk::manifest::ServerManifest;
use std::path::Path;

fn manifest() -> ServerManifest {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("mcp-server.toml");
    ServerManifest::from_file(&path).expect("manifest should parse")
}

#[test]
fn manifest_parses_and_validates() {
    let m = manifest();
    assert!(m.validate().is_empty(), "validation errors: {:?}", m.validate());
    assert_eq!(m.server_id, "mcp_loyalty");
    assert_eq!(m.tools.len(), 18, "expected 18 declared tools");
}

#[test]
fn points_mutations_are_gated() {
    let m = manifest();
    for name in ["earn_points", "award_points", "adjust_points", "expire_points", "redeem_reward", "set_redemption_status"] {
        let t = m.tools.iter().find(|t| t.name == name).unwrap_or_else(|| panic!("{name} present"));
        assert!(t.requires_approval, "{name} must require approval");
    }
}

#[test]
fn reads_are_read_only() {
    use adk_mcp_sdk::risk::RiskClass;
    let m = manifest();
    for name in ["get_member", "find_member", "get_balance", "get_ledger", "list_rewards", "get_redemptions"] {
        let t = m.tools.iter().find(|t| t.name == name).unwrap();
        assert_eq!(t.risk_class, RiskClass::ReadOnly, "{name} should be read_only");
    }
}
