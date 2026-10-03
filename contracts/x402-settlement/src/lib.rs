use parity_scale_codec::{Decode, Encode};
use qmoosa_primitives::AccountId32;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;

#[derive(Error, Debug, PartialEq, Eq, Encode, Decode)]
pub enum SettlementError {
    #[error("Invoice challenge already settled")]
    AlreadySettled,
    #[error("Payment value must be greater than zero")]
    ZeroPayment,
}

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, Serialize, Deserialize)]
pub enum X402SettlementMessage {
    SettlePayment {
        challenge_id: String,
        payee: AccountId32,
        amount: u128,
        now: i64,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, Serialize, Deserialize)]
pub enum X402SettlementEvent {
    PaymentSettled {
        challenge_id: String,
        payer: AccountId32,
        payee: AccountId32,
        gross_amount: u128,
        fee_amount: u128,
        net_amount: u128,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, Serialize, Deserialize)]
pub struct SettledInvoice {
    pub challenge_id: String,
    pub payer: AccountId32,
    pub payee: AccountId32,
    pub gross_amount: u128,
    pub fee_amount: u128,
    pub net_amount: u128,
    pub settled_at: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct X402SettlementRegistry {
    pub fee_collector: AccountId32,
    pub fee_bps: u128, // 100 = 1%
    pub invoices: HashMap<String, SettledInvoice>,
}

impl X402SettlementRegistry {
    pub fn new(fee_collector: AccountId32) -> Self {
        Self {
            fee_collector,
            fee_bps: 100,
            invoices: HashMap::new(),
        }
    }

    /// Primary PolkaVM dispatch entry point
    pub fn dispatch(
        &mut self,
        caller: AccountId32,
        msg: X402SettlementMessage,
    ) -> Result<X402SettlementEvent, SettlementError> {
        match msg {
            X402SettlementMessage::SettlePayment {
                challenge_id,
                payee,
                amount,
                now,
            } => {
                let invoice = self.settle_payment(&challenge_id, caller, payee, amount, now)?;
                Ok(X402SettlementEvent::PaymentSettled {
                    challenge_id: invoice.challenge_id,
                    payer: invoice.payer,
                    payee: invoice.payee,
                    gross_amount: invoice.gross_amount,
                    fee_amount: invoice.fee_amount,
                    net_amount: invoice.net_amount,
                })
            }
        }
    }

    pub fn settle_payment(
        &mut self,
        challenge_id: &str,
        payer: AccountId32,
        payee: AccountId32,
        amount: u128,
        now: i64,
    ) -> Result<SettledInvoice, SettlementError> {
        if amount == 0 {
            return Err(SettlementError::ZeroPayment);
        }
        if self.invoices.contains_key(challenge_id) {
            return Err(SettlementError::AlreadySettled);
        }

        let fee_amount = (amount * self.fee_bps) / 10000;
        let net_amount = amount - fee_amount;

        let record = SettledInvoice {
            challenge_id: challenge_id.to_string(),
            payer,
            payee,
            gross_amount: amount,
            fee_amount,
            net_amount,
            settled_at: now,
        };

        self.invoices
            .insert(challenge_id.to_string(), record.clone());
        Ok(record)
    }

    pub fn is_settled(&self, challenge_id: &str) -> bool {
        self.invoices.contains_key(challenge_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mock_account(id: u8) -> AccountId32 {
        AccountId32::new([id; 32])
    }

    #[test]
    fn test_settle_payment_pass() {
        let fee_collector = mock_account(99);
        let payer = mock_account(1);
        let payee = mock_account(2);

        let mut reg = X402SettlementRegistry::new(fee_collector);
        let now = 1000;

        let inv = reg
            .settle_payment("inv_001", payer, payee, 10_000, now)
            .unwrap();
        assert_eq!(inv.gross_amount, 10_000);
        assert_eq!(inv.fee_amount, 100); // 1% fee
        assert_eq!(inv.net_amount, 9900);
        assert_eq!(inv.payer, payer);
        assert_eq!(inv.payee, payee);
        assert!(reg.is_settled("inv_001"));
    }

    #[test]
    fn test_replay_settlement_fail() {
        let fee_collector = mock_account(99);
        let payer = mock_account(1);
        let payee = mock_account(2);

        let mut reg = X402SettlementRegistry::new(fee_collector);
        reg.settle_payment("inv_replay", payer, payee, 5000, 1000)
            .unwrap();

        let dup = reg.settle_payment("inv_replay", payer, payee, 5000, 1010);
        assert_eq!(dup, Err(SettlementError::AlreadySettled));
    }

    #[test]
    fn test_zero_payment_fail() {
        let fee_collector = mock_account(99);
        let payer = mock_account(1);
        let payee = mock_account(2);

        let mut reg = X402SettlementRegistry::new(fee_collector);
        assert_eq!(
            reg.settle_payment("inv_zero", payer, payee, 0, 1000),
            Err(SettlementError::ZeroPayment)
        );
    }

    #[test]
    fn test_polkavm_dispatch_and_event() {
        let fee_collector = mock_account(99);
        let payer = mock_account(1);
        let payee = mock_account(2);

        let mut reg = X402SettlementRegistry::new(fee_collector);
        let msg = X402SettlementMessage::SettlePayment {
            challenge_id: "inv_poly_01".to_string(),
            payee,
            amount: 50_000,
            now: 2000,
        };

        let encoded_msg = msg.encode();
        let decoded_msg = X402SettlementMessage::decode(&mut &encoded_msg[..]).unwrap();

        let event = reg.dispatch(payer, decoded_msg).unwrap();
        assert_eq!(
            event,
            X402SettlementEvent::PaymentSettled {
                challenge_id: "inv_poly_01".to_string(),
                payer,
                payee,
                gross_amount: 50_000,
                fee_amount: 500,
                net_amount: 49_500,
            }
        );
    }
}
