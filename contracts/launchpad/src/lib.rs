use std::collections::HashMap;
use thiserror::Error;

#[derive(Error, Debug, PartialEq)]
pub enum LaunchpadError {
    #[error("Campaign has not started yet")]
    NotStarted,
    #[error("Campaign has already ended")]
    Ended,
    #[error("Tokens are still locked in vesting")]
    VestingLocked,
    #[error("No tokens available to claim")]
    NoClaimableTokens,
    #[error("Campaign already finalized")]
    AlreadyFinalized,
    #[error("Unauthorized finalization")]
    Unauthorized,
    #[error("Exceeds campaign hardcap")]
    ExceedsCap,
}

pub struct Campaign {
    pub creator: String,
    pub token_price_dot: u128,
    pub tokens_for_sale: u128,
    pub tokens_sold: u128,
    pub total_dot_raised: u128,
    pub start_time: i64,
    pub end_time: i64,
    pub vesting_duration_secs: i64,
    pub finalized: bool,
}

pub struct InvestorAllocation {
    pub purchased_tokens: u128,
    pub claimed_tokens: u128,
    pub purchase_time: i64,
}

pub struct LaunchpadManager {
    pub treasury: String,
    pub treasury_fee_bps: u128, // 250 = 2.5%
    pub campaigns: HashMap<u64, Campaign>,
    pub allocations: HashMap<(u64, String), InvestorAllocation>,
    pub next_campaign_id: u64,
}

impl LaunchpadManager {
    pub fn new(treasury: &str) -> Self {
        Self {
            treasury: treasury.to_string(),
            treasury_fee_bps: 250,
            campaigns: HashMap::new(),
            allocations: HashMap::new(),
            next_campaign_id: 1,
        }
    }

    pub fn create_campaign(
        &mut self,
        creator: &str,
        token_price_dot: u128,
        tokens_for_sale: u128,
        duration_secs: i64,
        vesting_duration_secs: i64,
        now: i64,
    ) -> u64 {
        let id = self.next_campaign_id;
        self.next_campaign_id += 1;

        let campaign = Campaign {
            creator: creator.to_string(),
            token_price_dot,
            tokens_for_sale,
            tokens_sold: 0,
            total_dot_raised: 0,
            start_time: now,
            end_time: now + duration_secs,
            vesting_duration_secs,
            finalized: false,
        };

        self.campaigns.insert(id, campaign);
        id
    }

    pub fn buy_tokens(&mut self, campaign_id: u64, buyer: &str, dot_amount: u128, now: i64) -> Result<u128, LaunchpadError> {
        let camp = self.campaigns.get_mut(&campaign_id).ok_or(LaunchpadError::NotStarted)?;

        if now < camp.start_time {
            return Err(LaunchpadError::NotStarted);
        }
        if now > camp.end_time || camp.finalized {
            return Err(LaunchpadError::Ended);
        }

        let token_amount = (dot_amount * 1_000_000_000_000_000_000) / camp.token_price_dot;
        if camp.tokens_sold + token_amount > camp.tokens_for_sale {
            return Err(LaunchpadError::ExceedsCap);
        }

        camp.tokens_sold += token_amount;
        camp.total_dot_raised += dot_amount;

        let alloc = self.allocations.entry((campaign_id, buyer.to_string())).or_insert(InvestorAllocation {
            purchased_tokens: 0,
            claimed_tokens: 0,
            purchase_time: now,
        });

        alloc.purchased_tokens += token_amount;
        Ok(token_amount)
    }

    pub fn claim_tokens(&mut self, campaign_id: u64, claimer: &str, now: i64) -> Result<u128, LaunchpadError> {
        let camp = self.campaigns.get(&campaign_id).ok_or(LaunchpadError::NotStarted)?;
        let alloc = self.allocations.get_mut(&(campaign_id, claimer.to_string())).ok_or(LaunchpadError::NoClaimableTokens)?;

        if alloc.purchased_tokens <= alloc.claimed_tokens {
            return Err(LaunchpadError::NoClaimableTokens);
        }

        // Vesting check
        let unlock_time = camp.end_time + camp.vesting_duration_secs;
        if now < unlock_time {
            return Err(LaunchpadError::VestingLocked);
        }

        let claimable = alloc.purchased_tokens - alloc.claimed_tokens;
        alloc.claimed_tokens = alloc.purchased_tokens;

        Ok(claimable)
    }

    pub fn finalize_campaign(&mut self, campaign_id: u64, caller: &str) -> Result<(u128, u128), LaunchpadError> {
        let camp = self.campaigns.get_mut(&campaign_id).ok_or(LaunchpadError::NotStarted)?;

        if caller != camp.creator && caller != self.treasury {
            return Err(LaunchpadError::Unauthorized);
        }
        if camp.finalized {
            return Err(LaunchpadError::AlreadyFinalized);
        }

        camp.finalized = true;

        let fee = (camp.total_dot_raised * self.treasury_fee_bps) / 10000;
        let proceeds = camp.total_dot_raised - fee;

        Ok((proceeds, fee))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ONE_TOKEN: u128 = 1_000_000_000_000_000_000;

    #[test]
    fn test_buy_tokens_pass() {
        let mut lm = LaunchpadManager::new("treasury_wallet");
        let now = 1000;
        let camp_id = lm.create_campaign("creator", ONE_TOKEN, 10_000 * ONE_TOKEN, 3600, 1800, now);

        let purchased = lm.buy_tokens(camp_id, "buyer", 5 * ONE_TOKEN, now + 100).unwrap();
        assert_eq!(purchased, 5 * ONE_TOKEN);
    }

    #[test]
    fn test_early_claim_fail() {
        let mut lm = LaunchpadManager::new("treasury_wallet");
        let now = 1000;
        let camp_id = lm.create_campaign("creator", ONE_TOKEN, 10_000 * ONE_TOKEN, 3600, 1800, now);

        lm.buy_tokens(camp_id, "buyer", 5 * ONE_TOKEN, now + 100).unwrap();

        // Attempt claim before vesting expires (now + 3600 + 1800)
        let early_claim = lm.claim_tokens(camp_id, "buyer", now + 2000);
        assert_eq!(early_claim, Err(LaunchpadError::VestingLocked));
    }

    #[test]
    fn test_claim_after_vesting_pass() {
        let mut lm = LaunchpadManager::new("treasury_wallet");
        let now = 1000;
        let camp_id = lm.create_campaign("creator", ONE_TOKEN, 10_000 * ONE_TOKEN, 3600, 1800, now);

        lm.buy_tokens(camp_id, "buyer", 5 * ONE_TOKEN, now + 100).unwrap();

        // Claim after vesting unlock time (1000 + 3600 + 1800 + 10 = 6410)
        let claimed = lm.claim_tokens(camp_id, "buyer", 6410).unwrap();
        assert_eq!(claimed, 5 * ONE_TOKEN);

        // Double claim must fail
        assert_eq!(lm.claim_tokens(camp_id, "buyer", 6420), Err(LaunchpadError::NoClaimableTokens));
    }

    #[test]
    fn test_finalize_treasury_fee() {
        let mut lm = LaunchpadManager::new("treasury_wallet");
        let now = 1000;
        let camp_id = lm.create_campaign("creator", ONE_TOKEN, 10_000 * ONE_TOKEN, 3600, 1800, now);

        lm.buy_tokens(camp_id, "buyer", 10_000, now + 100).unwrap();

        let (proceeds, fee) = lm.finalize_campaign(camp_id, "creator").unwrap();
        // 2.5% fee on 10000 = 250
        assert_eq!(fee, 250);
        assert_eq!(proceeds, 9750);

        // Double finalize must fail
        assert_eq!(lm.finalize_campaign(camp_id, "creator"), Err(LaunchpadError::AlreadyFinalized));
    }
}
