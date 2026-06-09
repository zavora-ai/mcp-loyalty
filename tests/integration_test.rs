//! Integration tests for the loyalty points engine.

use mcp_loyalty::store::LoyaltyStore;
use mcp_loyalty::types::*;

fn store() -> LoyaltyStore {
    LoyaltyStore::new()
}

#[test]
fn seeded_members_present() {
    let s = store();
    assert!(!s.find_member(None, Some("alice")).is_empty());
    assert!(!s.find_member(None, Some("bob")).is_empty());
}

#[test]
fn enroll_starts_bronze_zero() {
    let s = store();
    let m = s.enroll("New Person".into(), "ref:new".into());
    assert_eq!(m.tier, Tier::Bronze);
    assert_eq!(m.points_balance, 0);
    assert!(m.membership_no.starts_with("LY"));
}

#[test]
fn earn_applies_tier_and_offer_multiplier() {
    let s = store();
    let m = s.enroll("Earner".into(), "ref:e".into());
    // Bronze x1.0, but seed has a "Double Points Weekend" offer (x2, all tiers).
    let e = s.earn_from_spend(&m.id, 100.0, None, "pos").unwrap();
    assert_eq!(e.points, 200, "100 spend x1.0 tier x2.0 offer = 200");
    assert_eq!(e.balance_after, 200);
}

#[test]
fn ledger_balance_is_consistent() {
    let s = store();
    let m = s.enroll("Ledger".into(), "ref:l".into());
    s.award_points(&m.id, 1000, "bonus", "sys").unwrap();
    s.adjust_points(&m.id, -300, "correction", "sys").unwrap();
    s.expire_points(&m.id, 200, "expiry", "sys").unwrap();
    let bal = s.get_member(&m.id).unwrap().points_balance;
    assert_eq!(bal, 500);
    // Last ledger entry's balance_after must equal the member balance.
    let last = &s.ledger_for(&m.id, 1)[0];
    assert_eq!(last.balance_after, 500);
}

#[test]
fn cannot_go_negative() {
    let s = store();
    let m = s.enroll("Broke".into(), "ref:b".into());
    s.award_points(&m.id, 100, "bonus", "sys").unwrap();
    assert!(s.adjust_points(&m.id, -500, "too much", "sys").is_err());
    assert!(s.expire_points(&m.id, 500, "too much", "sys").is_err());
    assert_eq!(s.get_member(&m.id).unwrap().points_balance, 100);
}

#[test]
fn tier_progresses_on_lifetime_points() {
    let s = store();
    let m = s.enroll("Climber".into(), "ref:c".into());
    s.award_points(&m.id, 1000, "to silver", "sys").unwrap();
    assert_eq!(s.get_member(&m.id).unwrap().tier, Tier::Silver);
    s.award_points(&m.id, 4000, "to gold", "sys").unwrap();
    assert_eq!(s.get_member(&m.id).unwrap().tier, Tier::Gold);
}

#[test]
fn redemptions_do_not_lower_tier() {
    let s = store();
    let m = s.enroll("Spender".into(), "ref:s".into());
    s.award_points(&m.id, 5000, "to gold", "sys").unwrap(); // Gold
    let r = s.create_reward("Big".into(), "".into(), 5000, None, None);
    s.redeem(&m.id, &r.id, "agent").unwrap();
    let after = s.get_member(&m.id).unwrap();
    assert_eq!(after.points_balance, 0);
    assert_eq!(after.tier, Tier::Gold, "redeeming spends balance but lifetime/tier stay");
}

#[test]
fn redeem_enforces_tier_and_balance() {
    let s = store();
    let m = s.enroll("Bronze".into(), "ref:br".into());
    s.award_points(&m.id, 10000, "pts", "sys").unwrap(); // lots of points but...
    // ...still Platinum? 10000 lifetime = Gold. Gold reward should pass; Platinum-only would fail.
    let gold_reward = s.create_reward("Gold perk".into(), "".into(), 100, Some(Tier::Gold), None);
    assert!(s.redeem(&m.id, &gold_reward.id, "a").is_ok());
    // Insufficient balance case.
    let pricey = s.create_reward("Pricey".into(), "".into(), 999999, None, None);
    assert!(s.redeem(&m.id, &pricey.id, "a").is_err());
}

#[test]
fn cancel_redemption_refunds_points() {
    let s = store();
    let m = s.enroll("Refund".into(), "ref:rf".into());
    s.award_points(&m.id, 1000, "pts", "sys").unwrap();
    let reward = s.create_reward("Voucher".into(), "".into(), 1000, None, Some(5));
    let red = s.redeem(&m.id, &reward.id, "a").unwrap();
    assert_eq!(s.get_member(&m.id).unwrap().points_balance, 0);
    s.set_redemption_status(&red.id, RedemptionStatus::Cancelled, "a").unwrap();
    assert_eq!(s.get_member(&m.id).unwrap().points_balance, 1000, "cancel refunds points");
    // inventory restored
    assert_eq!(s.get_reward(&reward.id).unwrap().inventory, Some(5));
}

#[test]
fn inventory_decrements_and_blocks_when_zero() {
    let s = store();
    let m = s.enroll("Inv".into(), "ref:i".into());
    s.award_points(&m.id, 10000, "pts", "sys").unwrap();
    let reward = s.create_reward("Limited".into(), "".into(), 100, None, Some(1));
    assert!(s.redeem(&m.id, &reward.id, "a").is_ok());
    assert_eq!(s.get_reward(&reward.id).unwrap().inventory, Some(0));
    assert!(s.redeem(&m.id, &reward.id, "a").is_err(), "out of stock");
}

#[test]
fn suspended_account_cannot_earn_or_redeem() {
    let s = store();
    let m = s.enroll("Susp".into(), "ref:su".into());
    s.award_points(&m.id, 1000, "pts", "sys").unwrap();
    s.set_member_status(&m.id, MemberStatus::Suspended).unwrap();
    assert!(s.earn_from_spend(&m.id, 100.0, None, "pos").is_err());
    let reward = s.create_reward("R".into(), "".into(), 100, None, None);
    assert!(s.redeem(&m.id, &reward.id, "a").is_err());
}
