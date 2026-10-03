use std::collections::HashMap;
use thiserror::Error;

#[derive(Error, Debug, PartialEq)]
pub enum SettlementError {
    #[error("Invoice challenge already settled")]
    AlreadySettled,
    #[error("Payment value must be greater than zero")]
    ZeroPayment,
    #[error("Invalid payee address")]
    InvalidPayee,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SettledInvoice {
    pub challenge_id: String,
    pub payer: String,
    pub payee: String,
    pub gross_amount: u128,
    pub fee_amount: u128,
    pub net_amount: u128,
    pub settled_at: i64,
}

pub struct X402SettlementRegistry {
    pub fee_collector: String,
    pub fee_bps: u128, // 100 = 1%
    pub invoices: HashMap<String, SettledInvoice>,
}

impl X402SettlementRegistry {
    pub fn new(fee_collector: &str) -> Self {
        Self {
            fee_collector: fee_collector.to_string(),
            fee_bps: 100,
            invoices: HashMap::new(),
        }
    }

    pub fn settle_payment(
        &mut self,
        challenge_id: &str,
        payer: &str,
        payee: &str,
        amount: u128,
        now: i64,
    ) -> Result<SettledInvoice, SettlementError> {
        if amount == 0 {
            return Err(SettlementError::ZeroPayment);
        }
        if payee.is_empty() {
            return Err(SettlementError::InvalidPayee);
        }
        if self.invoices.contains_key(challenge_id) {
            return Err(SettlementError::AlreadySettled);
        }

        let fee_amount = (amount * self.fee_bps) / 10000;
        let net_amount = amount - fee_amount;

        let record = SettledInvoice {
            challenge_id: challenge_id.to_string(),
            payer: payer.to_string(),
            payee: payee.to_string(),
            gross_amount: amount,
            fee_amount,
            net_amount,
            settled_at: now,
        };

        self.invoices.insert(challenge_id.to_string(), record.clone());
        Ok(record)
    }

    pub fn is_settled(&self, challenge_id: &str) -> bool {
        self.invoices.contains_key(challenge_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_settle_payment_pass() {
        let mut reg = X402SettlementRegistry::new("fee_collector_address");
        let invoice = reg.settle_payment("challenge_001", "payer_agent", "merchant_api", 10_000, 1000).unwrap();

        assert_eq!(invoice.fee_amount, 100); // 1%
        assert_eq!(invoice.net_amount, 9900); // 99%
        assert!(reg.is_settled("challenge_001"));
    }

    #[test]
    fn test_replay_settlement_fail() {
        let mut reg = X402SettlementRegistry::new("fee_collector_address");
        reg.settle_payment("challenge_replay", "payer", "payee", 5000, 1000).unwrap();

        // Second settlement attempt with same challenge_id must fail
        let res = reg.settle_payment("challenge_replay", "payer", "payee", 5000, 1001);
        assert_eq!(res, Err(SettlementError::AlreadySettled));
    }

    #[test]
    fn test_zero_payment_fail() {
        let mut reg = X402SettlementRegistry::new("fee_collector_address");
        let res = reg.settle_payment("challenge_zero", "payer", "payee", 0, 1000);
        assert_eq!(res, Err(SettlementError::ZeroPayment));
    }
}
