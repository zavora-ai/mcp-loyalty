use chrono::{DateTime, NaiveDate, Utc};
use rmcp::schemars;
use serde::{Deserialize, Serialize};

/// Membership tier. Ordered low→high; thresholds are lifetime earned points.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, schemars::JsonSchema, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum Tier {
    Bronze,
    Silver,
    Gold,
    Platinum,
}

impl Tier {
    /// Lifetime-earned-points threshold to reach this tier.
    pub fn threshold(self) -> i64 {
        match self {
            Tier::Bronze => 0,
            Tier::Silver => 1_000,
            Tier::Gold => 5_000,
            Tier::Platinum => 20_000,
        }
    }

    /// Earn multiplier granted by the tier (basis: 1.0 = base rate).
    pub fn earn_multiplier(self) -> f64 {
        match self {
            Tier::Bronze => 1.0,
            Tier::Silver => 1.25,
            Tier::Gold => 1.5,
            Tier::Platinum => 2.0,
        }
    }

    /// The tier earned by a given lifetime points total.
    pub fn from_lifetime(points: i64) -> Tier {
        if points >= Tier::Platinum.threshold() {
            Tier::Platinum
        } else if points >= Tier::Gold.threshold() {
            Tier::Gold
        } else if points >= Tier::Silver.threshold() {
            Tier::Silver
        } else {
            Tier::Bronze
        }
    }

    pub fn next(self) -> Option<Tier> {
        match self {
            Tier::Bronze => Some(Tier::Silver),
            Tier::Silver => Some(Tier::Gold),
            Tier::Gold => Some(Tier::Platinum),
            Tier::Platinum => None,
        }
    }
}

/// Account status.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, schemars::JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MemberStatus {
    Active,
    Inactive,
    Suspended,
    Closed,
}

/// Kind of points-ledger entry.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, schemars::JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LedgerKind {
    /// Points earned from a qualifying activity (positive).
    Earn,
    /// Points spent on a redemption (negative).
    Redeem,
    /// Manual adjustment by staff/agent (positive or negative).
    Adjust,
    /// Points expired (negative).
    Expire,
    /// Reversal of a prior entry (e.g. returned purchase).
    Reversal,
}

/// Reward redemption lifecycle.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, schemars::JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RedemptionStatus {
    Pending,
    Fulfilled,
    Cancelled,
}

/// A loyalty member account.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Member {
    pub id: String,
    /// Loyalty/membership number.
    pub membership_no: String,
    pub name: String,
    /// Contact reference (token/email-ref, not necessarily raw email).
    pub contact_ref: String,
    pub status: MemberStatus,
    pub tier: Tier,
    /// Current spendable balance (derived from the ledger).
    pub points_balance: i64,
    /// Lifetime points earned (drives tier; never decremented by redemptions).
    pub lifetime_points: i64,
    pub enrolled_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// An append-only points-ledger entry — the source of truth for balances.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct LedgerEntry {
    pub id: String,
    pub member_id: String,
    pub kind: LedgerKind,
    /// Signed points delta (earn/adjust+/reversal+ positive; redeem/expire negative).
    pub points: i64,
    /// Running balance after this entry.
    pub balance_after: i64,
    pub reason: String,
    /// Optional source reference (order id, campaign id, redemption id).
    pub reference: Option<String>,
    /// Expiry date for earned points (informational).
    pub expires_on: Option<NaiveDate>,
    pub actor: String,
    pub created_at: DateTime<Utc>,
}

/// A catalog reward that can be redeemed for points.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Reward {
    pub id: String,
    pub name: String,
    pub description: String,
    pub points_cost: i64,
    /// Minimum tier required to redeem (None = any tier).
    pub min_tier: Option<Tier>,
    /// Remaining inventory (None = unlimited).
    pub inventory: Option<i64>,
    pub active: bool,
}

/// A reward redemption.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Redemption {
    pub id: String,
    pub member_id: String,
    pub reward_id: String,
    pub reward_name: String,
    pub points_cost: i64,
    pub status: RedemptionStatus,
    pub created_at: DateTime<Utc>,
    pub fulfilled_at: Option<DateTime<Utc>>,
}

/// A promotional offer (e.g. bonus-points campaign).
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Offer {
    pub id: String,
    pub name: String,
    pub description: String,
    /// Bonus multiplier applied to earns while enrolled (e.g. 2.0 = double).
    pub bonus_multiplier: f64,
    /// Tiers the offer targets (empty = all).
    pub eligible_tiers: Vec<Tier>,
    pub start_date: Option<NaiveDate>,
    pub end_date: Option<NaiveDate>,
    pub active: bool,
}
