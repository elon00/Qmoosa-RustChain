use parity_scale_codec::{Decode, Encode};
use qmoosa_primitives::AccountId32;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;

#[derive(Error, Debug, PartialEq, Eq, Encode, Decode)]
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

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, Serialize, Deserialize)]
pub enum LaunchpadMessage {
    CreateCampaign {
        token_price_dot: u128,
        tokens_for_sale: u128,
        duration_secs: i64,
        vesting_duration_secs: i64,
        now: i64,
    },
    BuyTokens {
        campaign_id: u64,
        dot_amount: u128,
        now: i64,
    },
    ClaimTokens {
        campaign_id: u64,
        now: i64,
    },
    FinalizeCampaign {
        campaign_id: u64,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, Serialize, Deserialize)]
pub enum LaunchpadEvent {
    CampaignCreated {
        campaign_id: u64,
        creator: AccountId32,
        tokens_for_sale: u128,
    },
    TokensPurchased {
        campaign_id: u64,
        buyer: AccountId32,
        dot_amount: u128,
        token_amount: u128,
    },
    TokensClaimed {
        campaign_id: u64,
        claimer: AccountId32,
        token_amount: u128,
    },
    CampaignFinalized {
        campaign_id: u64,
        proceeds: u128,
        treasury_fee: u128,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Campaign {
    pub creator: AccountId32,
    pub token_price_dot: u128,
    pub tokens_for_sale: u128,
    pub tokens_sold: u128,
    pub total_dot_raised: u128,
    pub start_time: i64,
    pub end_time: i64,
    pub vesting_duration_secs: i64,
    pub finalized: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct InvestorAllocation {
    pub purchased_tokens: u128,
    pub claimed_tokens: u128,
    pub purchase_time: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LaunchpadManager {
    pub treasury: AccountId32,
    pub treasury_fee_bps: u128, // 250 = 2.5%
    pub campaigns: HashMap<u64, Campaign>,
    pub allocations: HashMap<(u64, AccountId32), InvestorAllocation>,
    pub next_campaign_id: u64,
}

impl LaunchpadManager {
    pub fn new(treasury: AccountId32) -> Self {
        Self {
            treasury,
            treasury_fee_bps: 250,
            campaigns: HashMap::new(),
            allocations: HashMap::new(),
            next_campaign_id: 1,
        }
    }

    /// Primary PolkaVM dispatch entry point
    pub fn dispatch(
        &mut self,
        caller: AccountId32,
        msg: LaunchpadMessage,
    ) -> Result<LaunchpadEvent, LaunchpadError> {
        match msg {
            LaunchpadMessage::CreateCampaign {
                token_price_dot,
                tokens_for_sale,
                duration_secs,
                vesting_duration_secs,
                now,
            } => {
                let id = self.create_campaign(
                    caller,
                    token_price_dot,
                    tokens_for_sale,
                    duration_secs,
                    vesting_duration_secs,
                    now,
                );
                Ok(LaunchpadEvent::CampaignCreated {
                    campaign_id: id,
                    creator: caller,
                    tokens_for_sale,
                })
            }
            LaunchpadMessage::BuyTokens {
                campaign_id,
                dot_amount,
                now,
            } => {
                let token_amount = self.buy_tokens(campaign_id, caller, dot_amount, now)?;
                Ok(LaunchpadEvent::TokensPurchased {
                    campaign_id,
                    buyer: caller,
                    dot_amount,
                    token_amount,
                })
            }
            LaunchpadMessage::ClaimTokens { campaign_id, now } => {
                let token_amount = self.claim_tokens(campaign_id, caller, now)?;
                Ok(LaunchpadEvent::TokensClaimed {
                    campaign_id,
                    claimer: caller,
                    token_amount,
                })
            }
            LaunchpadMessage::FinalizeCampaign { campaign_id } => {
                let (proceeds, treasury_fee) = self.finalize_campaign(campaign_id, caller)?;
                Ok(LaunchpadEvent::CampaignFinalized {
                    campaign_id,
                    proceeds,
                    treasury_fee,
                })
            }
        }
    }

    pub fn create_campaign(
        &mut self,
        creator: AccountId32,
        token_price_dot: u128,
        tokens_for_sale: u128,
        duration_secs: i64,
        vesting_duration_secs: i64,
        now: i64,
    ) -> u64 {
        let id = self.next_campaign_id;
        self.next_campaign_id += 1;

        let campaign = Campaign {
            creator,
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

    pub fn buy_tokens(
        &mut self,
        campaign_id: u64,
        buyer: AccountId32,
        dot_amount: u128,
        now: i64,
    ) -> Result<u128, LaunchpadError> {
        let camp = self
            .campaigns
            .get_mut(&campaign_id)
            .ok_or(LaunchpadError::NotStarted)?;

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

        let alloc = self
            .allocations
            .entry((campaign_id, buyer))
            .or_insert(InvestorAllocation {
                purchased_tokens: 0,
                claimed_tokens: 0,
                purchase_time: now,
            });

        alloc.purchased_tokens += token_amount;
        Ok(token_amount)
    }

    pub fn claim_tokens(
        &mut self,
        campaign_id: u64,
        claimer: AccountId32,
        now: i64,
    ) -> Result<u128, LaunchpadError> {
        let camp = self
            .campaigns
            .get(&campaign_id)
            .ok_or(LaunchpadError::NotStarted)?;
        let alloc = self
            .allocations
            .get_mut(&(campaign_id, claimer))
            .ok_or(LaunchpadError::NoClaimableTokens)?;

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

    pub fn finalize_campaign(
        &mut self,
        campaign_id: u64,
        caller: AccountId32,
    ) -> Result<(u128, u128), LaunchpadError> {
        let camp = self
            .campaigns
            .get_mut(&campaign_id)
            .ok_or(LaunchpadError::NotStarted)?;

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

    fn mock_account(id: u8) -> AccountId32 {
        AccountId32::new([id; 32])
    }

    #[test]
    fn test_buy_tokens_pass() {
        let treasury = mock_account(99);
        let creator = mock_account(1);
        let buyer = mock_account(2);

        let mut lm = LaunchpadManager::new(treasury);
        let now = 1000;
        let camp_id = lm.create_campaign(creator, ONE_TOKEN, 10_000 * ONE_TOKEN, 3600, 1800, now);

        let purchased = lm
            .buy_tokens(camp_id, buyer, 5 * ONE_TOKEN, now + 100)
            .unwrap();
        assert_eq!(purchased, 5 * ONE_TOKEN);
    }

    #[test]
    fn test_early_claim_fail() {
        let treasury = mock_account(99);
        let creator = mock_account(1);
        let buyer = mock_account(2);

        let mut lm = LaunchpadManager::new(treasury);
        let now = 1000;
        let camp_id = lm.create_campaign(creator, ONE_TOKEN, 10_000 * ONE_TOKEN, 3600, 1800, now);

        lm.buy_tokens(camp_id, buyer, 5 * ONE_TOKEN, now + 100)
            .unwrap();

        // Attempt claim before vesting expires (now + 3600 + 1800)
        let early_claim = lm.claim_tokens(camp_id, buyer, now + 2000);
        assert_eq!(early_claim, Err(LaunchpadError::VestingLocked));
    }

    #[test]
    fn test_claim_after_vesting_pass() {
        let treasury = mock_account(99);
        let creator = mock_account(1);
        let buyer = mock_account(2);

        let mut lm = LaunchpadManager::new(treasury);
        let now = 1000;
        let camp_id = lm.create_campaign(creator, ONE_TOKEN, 10_000 * ONE_TOKEN, 3600, 1800, now);

        lm.buy_tokens(camp_id, buyer, 5 * ONE_TOKEN, now + 100)
            .unwrap();

        // Claim after vesting unlock time (1000 + 3600 + 1800 + 10 = 6410)
        let claimed = lm.claim_tokens(camp_id, buyer, 6410).unwrap();
        assert_eq!(claimed, 5 * ONE_TOKEN);

        // Double claim must fail
        assert_eq!(
            lm.claim_tokens(camp_id, buyer, 6420),
            Err(LaunchpadError::NoClaimableTokens)
        );
    }

    #[test]
    fn test_finalize_treasury_fee() {
        let treasury = mock_account(99);
        let creator = mock_account(1);
        let buyer = mock_account(2);

        let mut lm = LaunchpadManager::new(treasury);
        let now = 1000;
        let camp_id = lm.create_campaign(creator, ONE_TOKEN, 10_000 * ONE_TOKEN, 3600, 1800, now);

        lm.buy_tokens(camp_id, buyer, 10_000, now + 100).unwrap();

        let (proceeds, fee) = lm.finalize_campaign(camp_id, creator).unwrap();
        // 2.5% fee on 10000 = 250
        assert_eq!(fee, 250);
        assert_eq!(proceeds, 9750);

        // Double finalize must fail
        assert_eq!(
            lm.finalize_campaign(camp_id, creator),
            Err(LaunchpadError::AlreadyFinalized)
        );
    }

    #[test]
    fn test_polkavm_dispatch_and_event() {
        let treasury = mock_account(99);
        let creator = mock_account(1);
        let buyer = mock_account(2);

        let mut lm = LaunchpadManager::new(treasury);
        let now = 1000;
        let camp_id = lm.create_campaign(creator, ONE_TOKEN, 10_000 * ONE_TOKEN, 3600, 1800, now);

        let msg = LaunchpadMessage::BuyTokens {
            campaign_id: camp_id,
            dot_amount: 2 * ONE_TOKEN,
            now: now + 50,
        };
        let encoded_msg = msg.encode();
        let decoded_msg = LaunchpadMessage::decode(&mut &encoded_msg[..]).unwrap();

        let event = lm.dispatch(buyer, decoded_msg).unwrap();
        assert_eq!(
            event,
            LaunchpadEvent::TokensPurchased {
                campaign_id: camp_id,
                buyer,
                dot_amount: 2 * ONE_TOKEN,
                token_amount: 2 * ONE_TOKEN,
            }
        );
    }
}
