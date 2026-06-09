use crate::store::LoyaltyStore;
use crate::types::*;
use adk_mcp_sdk::{HealthCheck, HealthStatus};
use rmcp::{handler::server::wrapper::Parameters, schemars, tool, tool_router};
use serde::Deserialize;
use std::sync::Arc;

// ── inputs ────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct EnrollInput { pub name: String, pub contact_ref: String }

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct IdInput { pub id: String }

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct FindMemberInput { pub membership_no: Option<String>, pub name: Option<String> }

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct SetMemberStatusInput { pub id: String, pub status: MemberStatus }

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct EarnInput { pub member_id: String, pub spend: f64, pub reference: Option<String>, #[serde(default = "default_actor")] pub actor: String }
fn default_actor() -> String { "agent".into() }

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct AwardInput { pub member_id: String, pub points: i64, pub reason: String, #[serde(default = "default_actor")] pub actor: String }

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct AdjustInput { pub member_id: String, pub points: i64, pub reason: String, #[serde(default = "default_actor")] pub actor: String }

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ExpireInput { pub member_id: String, pub points: i64, pub reason: String, #[serde(default = "default_actor")] pub actor: String }

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct LedgerInput { pub member_id: String, #[serde(default = "default_limit")] pub limit: usize }
fn default_limit() -> usize { 50 }

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct CreateRewardInput {
    pub name: String,
    #[serde(default)] pub description: String,
    pub points_cost: i64,
    pub min_tier: Option<Tier>,
    pub inventory: Option<i64>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ListRewardsInput { pub member_id: Option<String> }

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct RedeemInput { pub member_id: String, pub reward_id: String, #[serde(default = "default_actor")] pub actor: String }

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct SetRedemptionStatusInput { pub id: String, pub status: RedemptionStatus, #[serde(default = "default_actor")] pub actor: String }

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct MemberScopeInput { pub member_id: String }

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct CreateOfferInput { pub name: String, #[serde(default)] pub description: String, pub bonus_multiplier: f64, #[serde(default)] pub eligible_tiers: Vec<Tier> }

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ListOffersInput { #[serde(default)] pub active_only: bool }

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct SetOfferActiveInput { pub id: String, pub active: bool }

// ── server ──────────────────────────────────────────────────────────────

#[derive(Clone)]
pub struct LoyaltyServer { pub store: Arc<LoyaltyStore> }

#[tool_router(server_handler)]
impl LoyaltyServer {
    // ── members ───────────────────────────────────────────────────────────

    #[tool(description = "Enroll a new loyalty member. Returns the member id and membership number.")]
    fn enroll_member(&self, Parameters(i): Parameters<EnrollInput>) -> String {
        let m = self.store.enroll(i.name, i.contact_ref);
        serde_json::to_string_pretty(&serde_json::json!({"member_id": m.id, "membership_no": m.membership_no, "tier": m.tier, "status": m.status})).unwrap()
    }

    #[tool(description = "Get a member's account: tier, points balance, lifetime points, status.")]
    fn get_member(&self, Parameters(i): Parameters<IdInput>) -> String {
        match self.store.get_member(&i.id) {
            Some(m) => serde_json::to_string_pretty(&m).unwrap(),
            None => format!("Member not found: {}", i.id),
        }
    }

    #[tool(description = "Find members by membership number (exact) or name (contains).")]
    fn find_member(&self, Parameters(i): Parameters<FindMemberInput>) -> String {
        let ms = self.store.find_member(i.membership_no.as_deref(), i.name.as_deref());
        let out: Vec<serde_json::Value> = ms.iter().map(|m| serde_json::json!({"id": m.id, "membership_no": m.membership_no, "name": m.name, "tier": m.tier, "points_balance": m.points_balance})).collect();
        serde_json::to_string_pretty(&serde_json::json!({"count": out.len(), "members": out})).unwrap()
    }

    #[tool(description = "Change a member's account status (active/inactive/suspended/closed). Gated.")]
    fn set_member_status(&self, Parameters(i): Parameters<SetMemberStatusInput>) -> String {
        match self.store.set_member_status(&i.id, i.status) {
            Ok(m) => serde_json::to_string_pretty(&serde_json::json!({"id": m.id, "status": m.status})).unwrap(),
            Err(e) => format!("Error: {e}"),
        }
    }

    #[tool(description = "Get a member's points balance and tier progress (points to next tier).")]
    fn get_balance(&self, Parameters(i): Parameters<MemberScopeInput>) -> String {
        match self.store.tier_progress(&i.member_id) {
            Some(v) => serde_json::to_string_pretty(&v).unwrap(),
            None => format!("Member not found: {}", i.member_id),
        }
    }

    // ── points ────────────────────────────────────────────────────────────

    #[tool(description = "Earn points from a spend amount. Applies the member's tier multiplier and any active offer bonus, then posts to the ledger. Gated.")]
    fn earn_points(&self, Parameters(i): Parameters<EarnInput>) -> String {
        match self.store.earn_from_spend(&i.member_id, i.spend, i.reference, &i.actor) {
            Ok(e) => serde_json::to_string_pretty(&serde_json::json!({"ledger_id": e.id, "points_earned": e.points, "balance_after": e.balance_after})).unwrap(),
            Err(e) => format!("Error: {e}"),
        }
    }

    #[tool(description = "Award points directly (signup bonus, goodwill). Positive amount. Counts toward lifetime/tier. Gated.")]
    fn award_points(&self, Parameters(i): Parameters<AwardInput>) -> String {
        match self.store.award_points(&i.member_id, i.points, &i.reason, &i.actor) {
            Ok(e) => serde_json::to_string_pretty(&serde_json::json!({"ledger_id": e.id, "points": e.points, "balance_after": e.balance_after})).unwrap(),
            Err(e) => format!("Error: {e}"),
        }
    }

    #[tool(description = "Manually adjust points up or down (non-zero). Negative adjustments cannot drive the balance below zero. Gated.")]
    fn adjust_points(&self, Parameters(i): Parameters<AdjustInput>) -> String {
        match self.store.adjust_points(&i.member_id, i.points, &i.reason, &i.actor) {
            Ok(e) => serde_json::to_string_pretty(&serde_json::json!({"ledger_id": e.id, "points": e.points, "balance_after": e.balance_after})).unwrap(),
            Err(e) => format!("Error: {e}"),
        }
    }

    #[tool(description = "Expire a positive number of points from a member's balance. Gated.")]
    fn expire_points(&self, Parameters(i): Parameters<ExpireInput>) -> String {
        match self.store.expire_points(&i.member_id, i.points, &i.reason, &i.actor) {
            Ok(e) => serde_json::to_string_pretty(&serde_json::json!({"ledger_id": e.id, "points": e.points, "balance_after": e.balance_after})).unwrap(),
            Err(e) => format!("Error: {e}"),
        }
    }

    #[tool(description = "Get a member's points-ledger history (most recent first), the auditable source of truth for the balance.")]
    fn get_ledger(&self, Parameters(i): Parameters<LedgerInput>) -> String {
        if !self.store.member_exists(&i.member_id) {
            return format!("Member not found: {}", i.member_id);
        }
        let l = self.store.ledger_for(&i.member_id, i.limit);
        serde_json::to_string_pretty(&serde_json::json!({"member_id": i.member_id, "count": l.len(), "ledger": l})).unwrap()
    }

    // ── rewards & redemptions ─────────────────────────────────────────────

    #[tool(description = "Create a catalog reward (points cost, optional min tier and inventory).")]
    fn create_reward(&self, Parameters(i): Parameters<CreateRewardInput>) -> String {
        let r = self.store.create_reward(i.name, i.description, i.points_cost, i.min_tier, i.inventory);
        serde_json::to_string_pretty(&serde_json::json!({"reward_id": r.id, "name": r.name, "points_cost": r.points_cost})).unwrap()
    }

    #[tool(description = "List active rewards. If member_id is given, only rewards the member's tier qualifies for are returned.")]
    fn list_rewards(&self, Parameters(i): Parameters<ListRewardsInput>) -> String {
        let rs = self.store.list_rewards(i.member_id.as_deref());
        serde_json::to_string_pretty(&serde_json::json!({"count": rs.len(), "rewards": rs})).unwrap()
    }

    #[tool(description = "Redeem a reward for a member: validates tier, inventory, and balance; debits points via the ledger; decrements inventory; creates a redemption. Gated.")]
    fn redeem_reward(&self, Parameters(i): Parameters<RedeemInput>) -> String {
        match self.store.redeem(&i.member_id, &i.reward_id, &i.actor) {
            Ok(r) => serde_json::to_string_pretty(&serde_json::json!({"redemption_id": r.id, "reward": r.reward_name, "points_cost": r.points_cost, "status": r.status})).unwrap(),
            Err(e) => format!("Error: {e}"),
        }
    }

    #[tool(description = "Update a redemption's status (fulfilled or cancelled). Cancelling a pending redemption refunds the points and restores inventory. Gated.")]
    fn set_redemption_status(&self, Parameters(i): Parameters<SetRedemptionStatusInput>) -> String {
        match self.store.set_redemption_status(&i.id, i.status, &i.actor) {
            Ok(r) => serde_json::to_string_pretty(&serde_json::json!({"redemption_id": r.id, "status": r.status, "fulfilled_at": r.fulfilled_at})).unwrap(),
            Err(e) => format!("Error: {e}"),
        }
    }

    #[tool(description = "List a member's redemptions (most recent first).")]
    fn get_redemptions(&self, Parameters(i): Parameters<MemberScopeInput>) -> String {
        if !self.store.member_exists(&i.member_id) {
            return format!("Member not found: {}", i.member_id);
        }
        let rs = self.store.redemptions_for(&i.member_id);
        serde_json::to_string_pretty(&serde_json::json!({"count": rs.len(), "redemptions": rs})).unwrap()
    }

    // ── offers ───────────────────────────────────────────────────────────

    #[tool(description = "Create a promotional offer (bonus earn multiplier, optional tier targeting).")]
    fn create_offer(&self, Parameters(i): Parameters<CreateOfferInput>) -> String {
        let o = self.store.create_offer(i.name, i.description, i.bonus_multiplier, i.eligible_tiers);
        serde_json::to_string_pretty(&serde_json::json!({"offer_id": o.id, "name": o.name, "bonus_multiplier": o.bonus_multiplier})).unwrap()
    }

    #[tool(description = "List offers (set active_only=true for currently active ones).")]
    fn list_offers(&self, Parameters(i): Parameters<ListOffersInput>) -> String {
        let os = self.store.list_offers(i.active_only);
        serde_json::to_string_pretty(&serde_json::json!({"count": os.len(), "offers": os})).unwrap()
    }

    #[tool(description = "Activate or deactivate an offer. Gated.")]
    fn set_offer_active(&self, Parameters(i): Parameters<SetOfferActiveInput>) -> String {
        match self.store.set_offer_active(&i.id, i.active) {
            Ok(o) => serde_json::to_string_pretty(&serde_json::json!({"id": o.id, "active": o.active})).unwrap(),
            Err(e) => format!("Error: {e}"),
        }
    }
}

#[async_trait::async_trait]
impl HealthCheck for LoyaltyServer {
    async fn check_health(&self) -> HealthStatus {
        HealthStatus { healthy: true, message: Some("operational".into()), latency_ms: Some(1) }
    }
}
