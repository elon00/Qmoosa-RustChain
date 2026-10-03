use std::collections::{HashMap, HashSet};
use thiserror::Error;

#[derive(Error, Debug, PartialEq)]
pub enum TokenError {
    #[error("Caller is not authorized minter")]
    UnauthorizedMinter,
    #[error("Token transfers are currently paused")]
    TransfersPaused,
    #[error("Insufficient token balance")]
    InsufficientBalance,
    #[error("Insufficient allowance")]
    InsufficientAllowance,
    #[error("Burn amount exceeds balance")]
    BurnExceedsBalance,
}

pub struct QdotToken {
    pub name: String,
    pub symbol: String,
    pub decimals: u8,
    pub total_supply: u128,
    pub owner: String,
    pub paused: bool,
    pub balances: HashMap<String, u128>,
    pub allowances: HashMap<(String, String), u128>,
    pub minters: HashSet<String>,
}

impl QdotToken {
    pub fn new(name: &str, symbol: &str, decimals: u8, initial_supply: u128, owner: &str) -> Self {
        let mut balances = HashMap::new();
        let mut minters = HashSet::new();

        minters.insert(owner.to_string());
        if initial_supply > 0 {
            balances.insert(owner.to_string(), initial_supply);
        }

        Self {
            name: name.to_string(),
            symbol: symbol.to_string(),
            decimals,
            total_supply: initial_supply,
            owner: owner.to_string(),
            paused: false,
            balances,
            allowances: HashMap::new(),
            minters,
        }
    }

    pub fn set_minter(&mut self, caller: &str, account: &str, status: bool) -> Result<(), TokenError> {
        if caller != self.owner {
            return Err(TokenError::UnauthorizedMinter);
        }
        if status {
            self.minters.insert(account.to_string());
        } else {
            self.minters.remove(account);
        }
        Ok(())
    }

    pub fn set_paused(&mut self, caller: &str, paused: bool) -> Result<(), TokenError> {
        if caller != self.owner {
            return Err(TokenError::UnauthorizedMinter);
        }
        self.paused = paused;
        Ok(())
    }

    pub fn mint(&mut self, caller: &str, to: &str, amount: u128) -> Result<(), TokenError> {
        if !self.minters.contains(caller) && caller != self.owner {
            return Err(TokenError::UnauthorizedMinter);
        }
        self.total_supply = self.total_supply.saturating_add(amount);
        let bal = self.balances.entry(to.to_string()).or_insert(0);
        *bal = bal.saturating_add(amount);
        Ok(())
    }

    pub fn transfer(&mut self, from: &str, to: &str, amount: u128) -> Result<(), TokenError> {
        if self.paused {
            return Err(TokenError::TransfersPaused);
        }
        let from_bal = self.balances.entry(from.to_string()).or_insert(0);
        if *from_bal < amount {
            return Err(TokenError::InsufficientBalance);
        }
        *from_bal -= amount;

        let to_bal = self.balances.entry(to.to_string()).or_insert(0);
        *to_bal = to_bal.saturating_add(amount);
        Ok(())
    }

    pub fn burn(&mut self, caller: &str, amount: u128) -> Result<(), TokenError> {
        if self.paused {
            return Err(TokenError::TransfersPaused);
        }
        let bal = self.balances.entry(caller.to_string()).or_insert(0);
        if *bal < amount {
            return Err(TokenError::BurnExceedsBalance);
        }
        *bal -= amount;
        self.total_supply = self.total_supply.saturating_sub(amount);
        Ok(())
    }

    pub fn balance_of(&self, account: &str) -> u128 {
        *self.balances.get(account).unwrap_or(&0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mint_by_owner_and_minter_pass() {
        let mut token = QdotToken::new("Qmoosa Native Token", "QDOT", 18, 1_000_000, "owner");
        assert_eq!(token.balance_of("owner"), 1_000_000);

        // Assign minter
        assert!(token.set_minter("owner", "minter_agent", true).is_ok());

        // Minter mints new tokens (uncapped / flexible supply)
        assert!(token.mint("minter_agent", "user_1", 500_000).is_ok());
        assert_eq!(token.balance_of("user_1"), 500_000);
        assert_eq!(token.total_supply, 1_500_000);
    }

    #[test]
    fn test_unauthorized_mint_fail() {
        let mut token = QdotToken::new("Qmoosa Native Token", "QDOT", 18, 1_000_000, "owner");
        let result = token.mint("stranger", "stranger", 100_000);
        assert_eq!(result, Err(TokenError::UnauthorizedMinter));
    }

    #[test]
    fn test_transfer_when_paused_fail() {
        let mut token = QdotToken::new("Qmoosa Native Token", "QDOT", 18, 1_000_000, "owner");
        assert!(token.set_paused("owner", true).is_ok());

        let result = token.transfer("owner", "user_1", 100);
        assert_eq!(result, Err(TokenError::TransfersPaused));

        // Unpause
        assert!(token.set_paused("owner", false).is_ok());
        assert!(token.transfer("owner", "user_1", 100).is_ok());
        assert_eq!(token.balance_of("user_1"), 100);
    }

    #[test]
    fn test_burn_reduces_supply_pass() {
        let mut token = QdotToken::new("Qmoosa Native Token", "QDOT", 18, 1_000_000, "owner");
        assert!(token.burn("owner", 200_000).is_ok());
        assert_eq!(token.balance_of("owner"), 800_000);
        assert_eq!(token.total_supply, 800_000);
    }
}
