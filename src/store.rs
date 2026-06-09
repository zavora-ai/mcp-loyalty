use crate::types::*;
use chrono::{Duration, Utc};
use std::collections::HashMap;
use std::sync::Mutex;
use uuid::Uuid;

/// In-memory loyalty store. The points ledger is the source of truth for
/// balances; every change is posted through `post_ledger`. Sample data is
/// fictitious.
pub struct LoyaltyStore {
    members: Mutex<HashMap<String, Member>>,
    ledger: Mutex<Vec<LedgerEntry>>,
    rewards: Mutex<HashMap<String, Reward>>,
    redemptions: Mutex<HashMap<String, Redemption>>,
    offers: Mutex<HashMap<String, Offer>>,
    seq: Mutex<u64>,
    /// Base earn rate: points per unit of spend.
    base_rate: f64,
}

impl Default for LoyaltyStore {
    fn default() -> Self {
        Self::new()
    }
}

impl LoyaltyStore {
    pub fn new() -> Self {
        let s = Self {
            members: Mutex::new(HashMap::new()),
            ledger: Mutex::new(Vec::new()),
            rewards: Mutex::new(HashMap::new()),
            redemptions: Mutex::new(HashMap::new()),
            offers: Mutex::new(HashMap::new()),
            seq: Mutex::new(1000),
            base_rate: 1.0,
        };
        s.seed();
        s
    }

    fn next(&self, prefix: &str) -> String {
        let mut n = self.seq.lock().unwrap();
        *n += 1;
        format!("{prefix}-{}", *n)
    }

    // ── members ───────────────────────────────────────────────────────────

    pub fn enroll(&self, name: String, contact_ref: String) -> Member {
        let now = Utc::now();
        let m = Member {
            id: self.next("MBR"),
            membership_no: format!("LY{}", &Uuid::new_v4().simple().to_string()[..10].to_uppercase()),
            name,
            contact_ref,
            status: MemberStatus::Active,
            tier: Tier::Bronze,
            points_balance: 0,
            lifetime_points: 0,
            enrolled_at: now,
            updated_at: now,
        };
        self.members.lock().unwrap().insert(m.id.clone(), m.clone());
        m
    }

    pub fn get_member(&self, id: &str) -> Option<Member> {
        self.members.lock().unwrap().get(id).cloned()
    }

    pub fn find_member(&self, membership_no: Option<&str>, name: Option<&str>) -> Vec<Member> {
        let name_l = name.map(|n| n.to_lowercase());
        let mut v: Vec<Member> = self
            .members
            .lock()
            .unwrap()
            .values()
            .filter(|m| {
                membership_no.is_none_or(|no| m.membership_no.eq_ignore_ascii_case(no))
                    && name_l.as_ref().is_none_or(|n| m.name.to_lowercase().contains(n))
            })
            .cloned()
            .collect();
        v.sort_by(|a, b| a.id.cmp(&b.id));
        v
    }

    pub fn set_member_status(&self, id: &str, status: MemberStatus) -> Result<Member, String> {
        let mut ms = self.members.lock().unwrap();
        let m = ms.get_mut(id).ok_or_else(|| format!("Member not found: {id}"))?;
        m.status = status;
        m.updated_at = Utc::now();
        Ok(m.clone())
    }

    pub fn member_exists(&self, id: &str) -> bool {
        self.members.lock().unwrap().contains_key(id)
    }

    // ── ledger (the engine) ──────────────────────────────────────────────

    /// Post a ledger entry. Updates balance, lifetime points (on positive
    /// earn/adjust/reversal), and recomputes tier. Rejects moves that would
    /// drive the balance negative. Returns the entry.
    #[allow(clippy::too_many_arguments)]
    fn post_ledger(
        &self,
        member_id: &str,
        kind: LedgerKind,
        points: i64,
        reason: &str,
        reference: Option<String>,
        expires_days: Option<i64>,
        actor: &str,
    ) -> Result<LedgerEntry, String> {
        let mut members = self.members.lock().unwrap();
        let m = members.get_mut(member_id).ok_or_else(|| format!("Member not found: {member_id}"))?;
        if matches!(m.status, MemberStatus::Closed | MemberStatus::Suspended) && points > 0 {
            return Err(format!("Cannot credit points to a {:?} account", m.status));
        }
        let new_balance = m.points_balance + points;
        if new_balance < 0 {
            return Err(format!("Insufficient points: balance {} cannot absorb {}", m.points_balance, points));
        }
        m.points_balance = new_balance;
        // Lifetime only grows on genuinely earned/added points (not redemptions/expiry).
        if points > 0 && matches!(kind, LedgerKind::Earn | LedgerKind::Adjust | LedgerKind::Reversal) {
            m.lifetime_points += points;
            let new_tier = Tier::from_lifetime(m.lifetime_points);
            if new_tier != m.tier {
                m.tier = new_tier;
            }
        }
        m.updated_at = Utc::now();

        let entry = LedgerEntry {
            id: self.next("LG"),
            member_id: member_id.to_string(),
            kind,
            points,
            balance_after: new_balance,
            reason: reason.to_string(),
            reference,
            expires_on: expires_days.map(|d| (Utc::now() + Duration::days(d)).date_naive()),
            actor: actor.to_string(),
            created_at: Utc::now(),
        };
        self.ledger.lock().unwrap().push(entry.clone());
        Ok(entry)
    }

    /// Earn points from spend (applies tier multiplier + active offer bonuses).
    pub fn earn_from_spend(&self, member_id: &str, spend: f64, reference: Option<String>, actor: &str) -> Result<LedgerEntry, String> {
        if spend <= 0.0 {
            return Err("spend must be positive".into());
        }
        let tier = self.get_member(member_id).ok_or_else(|| format!("Member not found: {member_id}"))?.tier;
        let offer_bonus = self.best_offer_multiplier(tier);
        let pts = (spend * self.base_rate * tier.earn_multiplier() * offer_bonus).floor() as i64;
        self.post_ledger(member_id, LedgerKind::Earn, pts, &format!("Earn on spend {spend:.2} (x{:.2} tier, x{:.2} offer)", tier.earn_multiplier(), offer_bonus), reference, Some(365), actor)
    }

    /// Directly credit/award points (e.g. signup bonus, goodwill).
    pub fn award_points(&self, member_id: &str, points: i64, reason: &str, actor: &str) -> Result<LedgerEntry, String> {
        if points <= 0 {
            return Err("points must be positive".into());
        }
        self.post_ledger(member_id, LedgerKind::Adjust, points, reason, None, Some(365), actor)
    }

    /// Manual adjustment (positive or negative).
    pub fn adjust_points(&self, member_id: &str, points: i64, reason: &str, actor: &str) -> Result<LedgerEntry, String> {
        if points == 0 {
            return Err("points must be non-zero".into());
        }
        self.post_ledger(member_id, LedgerKind::Adjust, points, reason, None, None, actor)
    }

    pub fn expire_points(&self, member_id: &str, points: i64, reason: &str, actor: &str) -> Result<LedgerEntry, String> {
        if points <= 0 {
            return Err("expire amount must be positive".into());
        }
        self.post_ledger(member_id, LedgerKind::Expire, -points, reason, None, None, actor)
    }

    pub fn ledger_for(&self, member_id: &str, limit: usize) -> Vec<LedgerEntry> {
        let mut v: Vec<LedgerEntry> = self.ledger.lock().unwrap().iter().filter(|e| e.member_id == member_id).cloned().collect();
        v.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        v.truncate(limit);
        v
    }

    // ── rewards & redemptions ─────────────────────────────────────────────

    pub fn create_reward(&self, name: String, description: String, points_cost: i64, min_tier: Option<Tier>, inventory: Option<i64>) -> Reward {
        let r = Reward { id: self.next("RWD"), name, description, points_cost, min_tier, inventory, active: true };
        self.rewards.lock().unwrap().insert(r.id.clone(), r.clone());
        r
    }

    pub fn list_rewards(&self, member_id: Option<&str>) -> Vec<Reward> {
        let member_tier = member_id.and_then(|id| self.get_member(id)).map(|m| m.tier);
        let mut v: Vec<Reward> = self
            .rewards
            .lock()
            .unwrap()
            .values()
            .filter(|r| r.active)
            .filter(|r| member_tier.is_none_or(|mt| r.min_tier.is_none_or(|req| mt >= req)))
            .cloned()
            .collect();
        v.sort_by(|a, b| a.points_cost.cmp(&b.points_cost));
        v
    }

    pub fn get_reward(&self, id: &str) -> Option<Reward> {
        self.rewards.lock().unwrap().get(id).cloned()
    }

    /// Redeem a reward: validates tier + inventory + balance, debits points,
    /// decrements inventory, and records a redemption.
    pub fn redeem(&self, member_id: &str, reward_id: &str, actor: &str) -> Result<Redemption, String> {
        let member = self.get_member(member_id).ok_or_else(|| format!("Member not found: {member_id}"))?;
        if member.status != MemberStatus::Active {
            return Err(format!("Account is {:?}; cannot redeem", member.status));
        }
        let reward = self.get_reward(reward_id).ok_or_else(|| format!("Reward not found: {reward_id}"))?;
        if !reward.active {
            return Err("Reward is not active".into());
        }
        if let Some(req) = reward.min_tier {
            if member.tier < req {
                return Err(format!("Reward requires tier {:?}; member is {:?}", req, member.tier));
            }
        }
        if member.points_balance < reward.points_cost {
            return Err(format!("Insufficient points: need {}, have {}", reward.points_cost, member.points_balance));
        }
        // Inventory check + decrement.
        {
            let mut rewards = self.rewards.lock().unwrap();
            let r = rewards.get_mut(reward_id).unwrap();
            if let Some(inv) = r.inventory {
                if inv <= 0 {
                    return Err("Reward out of stock".into());
                }
                r.inventory = Some(inv - 1);
            }
        }
        // Debit points via the ledger.
        let redemption_id = self.next("RDM");
        self.post_ledger(member_id, LedgerKind::Redeem, -reward.points_cost, &format!("Redeemed: {}", reward.name), Some(redemption_id.clone()), None, actor)?;
        let red = Redemption {
            id: redemption_id,
            member_id: member_id.to_string(),
            reward_id: reward_id.to_string(),
            reward_name: reward.name.clone(),
            points_cost: reward.points_cost,
            status: RedemptionStatus::Pending,
            created_at: Utc::now(),
            fulfilled_at: None,
        };
        self.redemptions.lock().unwrap().insert(red.id.clone(), red.clone());
        Ok(red)
    }

    pub fn set_redemption_status(&self, id: &str, status: RedemptionStatus, actor: &str) -> Result<Redemption, String> {
        // On cancel, refund the points and restore inventory.
        let (member_id, reward_id, cost, was_cancelled) = {
            let mut reds = self.redemptions.lock().unwrap();
            let r = reds.get_mut(id).ok_or_else(|| format!("Redemption not found: {id}"))?;
            if r.status == status {
                return Err(format!("Redemption already {:?}", status));
            }
            let cancelling = status == RedemptionStatus::Cancelled && r.status == RedemptionStatus::Pending;
            r.status = status;
            if status == RedemptionStatus::Fulfilled {
                r.fulfilled_at = Some(Utc::now());
            }
            (r.member_id.clone(), r.reward_id.clone(), r.points_cost, cancelling)
        };
        if was_cancelled {
            // Refund points and restore inventory.
            let _ = self.post_ledger(&member_id, LedgerKind::Reversal, cost, &format!("Refund for cancelled redemption {id}"), Some(id.to_string()), None, actor);
            if let Some(r) = self.rewards.lock().unwrap().get_mut(&reward_id) {
                if let Some(inv) = r.inventory {
                    r.inventory = Some(inv + 1);
                }
            }
        }
        Ok(self.redemptions.lock().unwrap().get(id).cloned().unwrap())
    }

    pub fn redemptions_for(&self, member_id: &str) -> Vec<Redemption> {
        let mut v: Vec<Redemption> = self.redemptions.lock().unwrap().values().filter(|r| r.member_id == member_id).cloned().collect();
        v.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        v
    }

    // ── offers ───────────────────────────────────────────────────────────

    pub fn create_offer(&self, name: String, description: String, bonus_multiplier: f64, eligible_tiers: Vec<Tier>) -> Offer {
        let o = Offer { id: self.next("OFR"), name, description, bonus_multiplier, eligible_tiers, start_date: None, end_date: None, active: true };
        self.offers.lock().unwrap().insert(o.id.clone(), o.clone());
        o
    }

    pub fn list_offers(&self, active_only: bool) -> Vec<Offer> {
        let mut v: Vec<Offer> = self.offers.lock().unwrap().values().filter(|o| !active_only || o.active).cloned().collect();
        v.sort_by(|a, b| a.id.cmp(&b.id));
        v
    }

    pub fn set_offer_active(&self, id: &str, active: bool) -> Result<Offer, String> {
        let mut offers = self.offers.lock().unwrap();
        let o = offers.get_mut(id).ok_or_else(|| format!("Offer not found: {id}"))?;
        o.active = active;
        Ok(o.clone())
    }

    /// Best bonus multiplier across active offers eligible for `tier` (1.0 if none).
    fn best_offer_multiplier(&self, tier: Tier) -> f64 {
        self.offers
            .lock()
            .unwrap()
            .values()
            .filter(|o| o.active)
            .filter(|o| o.eligible_tiers.is_empty() || o.eligible_tiers.contains(&tier))
            .map(|o| o.bonus_multiplier)
            .fold(1.0, f64::max)
    }

    // ── tier projection ───────────────────────────────────────────────────

    /// Points to the next tier and progress, for a member.
    pub fn tier_progress(&self, member_id: &str) -> Option<serde_json::Value> {
        let m = self.get_member(member_id)?;
        let next = m.tier.next();
        let (to_next, next_name) = match next {
            Some(t) => ((t.threshold() - m.lifetime_points).max(0), Some(format!("{:?}", t).to_lowercase())),
            None => (0, None),
        };
        Some(serde_json::json!({
            "member_id": m.id,
            "tier": format!("{:?}", m.tier).to_lowercase(),
            "lifetime_points": m.lifetime_points,
            "points_balance": m.points_balance,
            "earn_multiplier": m.tier.earn_multiplier(),
            "next_tier": next_name,
            "points_to_next_tier": to_next,
        }))
    }

    // ── seed ────────────────────────────────────────────────────────────

    fn seed(&self) {
        let alice = self.enroll("Alice Wanjiru".into(), "ref:alice".into());
        let _ = self.earn_from_spend(&alice.id, 1200.0, Some("order:1001".into()), "pos");
        let bob = self.enroll("Bob Otieno".into(), "ref:bob".into());
        let _ = self.award_points(&bob.id, 6000, "Migrated balance", "system");

        self.create_reward("$10 Voucher".into(), "Store credit".into(), 1000, None, Some(500));
        self.create_reward("Free Shipping".into(), "One free delivery".into(), 500, None, None);
        self.create_reward("Gold Lounge Pass".into(), "Airport lounge access".into(), 8000, Some(Tier::Gold), Some(50));

        self.create_offer("Double Points Weekend".into(), "2x points on all earns".into(), 2.0, vec![]);
    }
}
