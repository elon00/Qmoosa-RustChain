use parity_scale_codec::{Decode, Encode};
use qmoosa_primitives::AccountId32;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use thiserror::Error;

#[derive(Error, Debug, PartialEq, Eq, Encode, Decode)]
pub enum TokenError {
    #[error("Caller is not authorized minter or owner")]
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

/// Standard ABI messages dispatched by PolkaVM runtime
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, Serialize, Deserialize)]
pub enum QdotMessage {
    Transfer {
        to: AccountId32,
        amount: u128,
    },
    TransferFrom {
        from: AccountId32,
        to: AccountId32,
        amount: u128,
    },
    Approve {
        spender: AccountId32,
        amount: u128,
    },
    Mint {
        to: AccountId32,
        amount: u128,
    },
    Burn {
        amount: u128,
    },
    SetMinter {
        account: AccountId32,
        status: bool,
    },
    SetPaused {
        paused: bool,
    },
}

/// SCALE-encoded events emitted to Polkadot host environment
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, Serialize, Deserialize)]
pub enum QdotEvent {
    Transfer {
        from: AccountId32,
        to: AccountId32,
        amount: u128,
    },
    Approval {
        owner: AccountId32,
        spender: AccountId32,
        amount: u128,
    },
    Mint {
        to: AccountId32,
        amount: u128,
    },
    Burn {
        from: AccountId32,
        amount: u128,
    },
    MinterUpdated {
        account: AccountId32,
        status: bool,
    },
    PauseStatusChanged {
        paused: bool,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct QdotToken {
    pub name: String,
    pub symbol: String,
    pub decimals: u8,
    pub total_supply: u128,
    pub owner: AccountId32,
    pub paused: bool,
    pub balances: HashMap<AccountId32, u128>,
    pub allowances: HashMap<(AccountId32, AccountId32), u128>,
    pub minters: HashSet<AccountId32>,
}

impl QdotToken {
    pub fn new(
        name: &str,
        symbol: &str,
        decimals: u8,
        initial_supply: u128,
        owner: AccountId32,
    ) -> Self {
        let mut balances = HashMap::new();
        let mut minters = HashSet::new();

        minters.insert(owner);
        if initial_supply > 0 {
            balances.insert(owner, initial_supply);
        }

        Self {
            name: name.to_string(),
            symbol: symbol.to_string(),
            decimals,
            total_supply: initial_supply,
            owner,
            paused: false,
            balances,
            allowances: HashMap::new(),
            minters,
        }
    }

    /// Primary PolkaVM dispatch entry point
    pub fn dispatch(
        &mut self,
        caller: AccountId32,
        msg: QdotMessage,
    ) -> Result<Option<QdotEvent>, TokenError> {
        match msg {
            QdotMessage::Transfer { to, amount } => {
                self.transfer(caller, to, amount)?;
                Ok(Some(QdotEvent::Transfer {
                    from: caller,
                    to,
                    amount,
                }))
            }
            QdotMessage::TransferFrom { from, to, amount } => {
                self.transfer_from(caller, from, to, amount)?;
                Ok(Some(QdotEvent::Transfer { from, to, amount }))
            }
            QdotMessage::Approve { spender, amount } => {
                self.approve(caller, spender, amount);
                Ok(Some(QdotEvent::Approval {
                    owner: caller,
                    spender,
                    amount,
                }))
            }
            QdotMessage::Mint { to, amount } => {
                self.mint(caller, to, amount)?;
                Ok(Some(QdotEvent::Mint { to, amount }))
            }
            QdotMessage::Burn { amount } => {
                self.burn(caller, amount)?;
                Ok(Some(QdotEvent::Burn {
                    from: caller,
                    amount,
                }))
            }
            QdotMessage::SetMinter { account, status } => {
                self.set_minter(caller, account, status)?;
                Ok(Some(QdotEvent::MinterUpdated { account, status }))
            }
            QdotMessage::SetPaused { paused } => {
                self.set_paused(caller, paused)?;
                Ok(Some(QdotEvent::PauseStatusChanged { paused }))
            }
        }
    }

    pub fn balance_of(&self, account: &AccountId32) -> u128 {
        *self.balances.get(account).unwrap_or(&0)
    }

    pub fn set_minter(
        &mut self,
        caller: AccountId32,
        account: AccountId32,
        status: bool,
    ) -> Result<(), TokenError> {
        if caller != self.owner {
            return Err(TokenError::UnauthorizedMinter);
        }
        if status {
            self.minters.insert(account);
        } else {
            self.minters.remove(&account);
        }
        Ok(())
    }

    pub fn set_paused(&mut self, caller: AccountId32, paused: bool) -> Result<(), TokenError> {
        if caller != self.owner {
            return Err(TokenError::UnauthorizedMinter);
        }
        self.paused = paused;
        Ok(())
    }

    pub fn mint(
        &mut self,
        caller: AccountId32,
        to: AccountId32,
        amount: u128,
    ) -> Result<(), TokenError> {
        if !self.minters.contains(&caller) && caller != self.owner {
            return Err(TokenError::UnauthorizedMinter);
        }
        self.total_supply = self.total_supply.saturating_add(amount);
        let bal = self.balances.entry(to).or_insert(0);
        *bal = bal.saturating_add(amount);
        Ok(())
    }

    pub fn transfer(
        &mut self,
        from: AccountId32,
        to: AccountId32,
        amount: u128,
    ) -> Result<(), TokenError> {
        if self.paused {
            return Err(TokenError::TransfersPaused);
        }
        let from_bal = self.balances.entry(from).or_insert(0);
        if *from_bal < amount {
            return Err(TokenError::InsufficientBalance);
        }
        *from_bal -= amount;

        let to_bal = self.balances.entry(to).or_insert(0);
        *to_bal = to_bal.saturating_add(amount);
        Ok(())
    }

    pub fn approve(&mut self, owner: AccountId32, spender: AccountId32, amount: u128) {
        self.allowances.insert((owner, spender), amount);
    }

    pub fn transfer_from(
        &mut self,
        spender: AccountId32,
        from: AccountId32,
        to: AccountId32,
        amount: u128,
    ) -> Result<(), TokenError> {
        if self.paused {
            return Err(TokenError::TransfersPaused);
        }
        let allowance = self.allowances.entry((from, spender)).or_insert(0);
        if *allowance < amount {
            return Err(TokenError::InsufficientAllowance);
        }
        *allowance -= amount;

        let from_bal = self.balances.entry(from).or_insert(0);
        if *from_bal < amount {
            return Err(TokenError::InsufficientBalance);
        }
        *from_bal -= amount;

        let to_bal = self.balances.entry(to).or_insert(0);
        *to_bal = to_bal.saturating_add(amount);
        Ok(())
    }

    pub fn burn(&mut self, caller: AccountId32, amount: u128) -> Result<(), TokenError> {
        if self.paused {
            return Err(TokenError::TransfersPaused);
        }
        let bal = self.balances.entry(caller).or_insert(0);
        if *bal < amount {
            return Err(TokenError::BurnExceedsBalance);
        }
        *bal -= amount;
        self.total_supply = self.total_supply.saturating_sub(amount);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mock_account(id: u8) -> AccountId32 {
        AccountId32::new([id; 32])
    }

    #[test]
    fn test_mint_by_owner_and_minter_pass() {
        let owner = mock_account(1);
        let alice = mock_account(2);
        let bob = mock_account(3);

        let mut token = QdotToken::new("Qmoosa DOT", "QDOT", 18, 1_000_000, owner);
        assert_eq!(token.balance_of(&owner), 1_000_000);

        // Owner mints to Alice
        assert!(token.mint(owner, alice, 500_000).is_ok());
        assert_eq!(token.balance_of(&alice), 500_000);

        // Alice is made a minter by owner and mints to Bob
        token.set_minter(owner, alice, true).unwrap();
        assert!(token.mint(alice, bob, 250_000).is_ok());
        assert_eq!(token.balance_of(&bob), 250_000);
        assert_eq!(token.total_supply, 1_750_000);
    }

    #[test]
    fn test_unauthorized_mint_fail() {
        let owner = mock_account(1);
        let attacker = mock_account(9);
        let mut token = QdotToken::new("Qmoosa DOT", "QDOT", 18, 100, owner);

        assert_eq!(
            token.mint(attacker, attacker, 1_000_000),
            Err(TokenError::UnauthorizedMinter)
        );
    }

    #[test]
    fn test_transfer_when_paused_fail() {
        let owner = mock_account(1);
        let alice = mock_account(2);
        let mut token = QdotToken::new("Qmoosa DOT", "QDOT", 18, 1_000, owner);

        token.set_paused(owner, true).unwrap();
        assert_eq!(
            token.transfer(owner, alice, 100),
            Err(TokenError::TransfersPaused)
        );

        token.set_paused(owner, false).unwrap();
        assert!(token.transfer(owner, alice, 100).is_ok());
        assert_eq!(token.balance_of(&alice), 100);
    }

    #[test]
    fn test_burn_reduces_supply_pass() {
        let owner = mock_account(1);
        let mut token = QdotToken::new("Qmoosa DOT", "QDOT", 18, 10_000, owner);

        token.burn(owner, 2_000).unwrap();
        assert_eq!(token.total_supply, 8_000);
        assert_eq!(token.balance_of(&owner), 8_000);
    }

    #[test]
    fn test_polkavm_dispatch_and_events() {
        let owner = mock_account(1);
        let bob = mock_account(2);
        let mut token = QdotToken::new("Qmoosa DOT", "QDOT", 18, 10_000, owner);

        let msg = QdotMessage::Transfer {
            to: bob,
            amount: 2500,
        };
        let encoded_msg = msg.encode();
        let decoded_msg = QdotMessage::decode(&mut &encoded_msg[..]).unwrap();

        let event = token.dispatch(owner, decoded_msg).unwrap();
        assert_eq!(
            event,
            Some(QdotEvent::Transfer {
                from: owner,
                to: bob,
                amount: 2500
            })
        );
        assert_eq!(token.balance_of(&bob), 2500);
    }
}
