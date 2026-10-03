use qmoosa_primitives::AccountId32;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ActionType {
    LaunchToken,
    X402Settle,
    QueryBalance,
    GeneralChat,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionProposal {
    pub target: AccountId32,
    pub value_plancks: u128,
    pub call_data_hex: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentActionProposal {
    pub proposal_id: String,
    pub action_type: ActionType,
    pub summary: String,
    pub requires_wallet_signature: bool,
    pub tx_proposal: Option<TransactionProposal>,
}

pub struct AgentOrchestrator {
    pub default_model: String,
    pub factory_account: AccountId32,
    pub x402_settlement_account: AccountId32,
}

impl AgentOrchestrator {
    pub fn new(
        default_model: &str,
        factory_account: AccountId32,
        x402_settlement_account: AccountId32,
    ) -> Self {
        Self {
            default_model: default_model.to_string(),
            factory_account,
            x402_settlement_account,
        }
    }

    pub fn process_intent(&self, user_prompt: &str) -> AgentActionProposal {
        let lower = user_prompt.to_lowercase();
        let proposal_id = format!("prop_{}", chrono::Utc::now().timestamp_millis());

        if lower.contains("launch token") || lower.contains("create token") {
            AgentActionProposal {
                proposal_id,
                action_type: ActionType::LaunchToken,
                summary: "Deploying new Polkadot Native dynamic uncapped token via PolkaVM"
                    .to_string(),
                requires_wallet_signature: true,
                tx_proposal: Some(TransactionProposal {
                    target: self.factory_account,
                    value_plancks: 0,
                    call_data_hex: "0xcreateToken".to_string(),
                }),
            }
        } else if lower.contains("x402") || lower.contains("pay api") {
            AgentActionProposal {
                proposal_id,
                action_type: ActionType::X402Settle,
                summary: "Settling HTTP 402 challenge on Polkadot Asset Hub".to_string(),
                requires_wallet_signature: true,
                tx_proposal: Some(TransactionProposal {
                    target: self.x402_settlement_account,
                    value_plancks: 50_000_000_000, // 0.05 DOT in 10-decimal plancks
                    call_data_hex: "0xsettleDot".to_string(),
                }),
            }
        } else {
            AgentActionProposal {
                proposal_id,
                action_type: ActionType::GeneralChat,
                summary: format!(
                    "Processed query with {}: '{}'",
                    self.default_model, user_prompt
                ),
                requires_wallet_signature: false,
                tx_proposal: None,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mock_account(id: u8) -> AccountId32 {
        AccountId32::new([id; 32])
    }

    #[test]
    fn test_intent_launch_token_proposal() {
        let factory = mock_account(1);
        let settlement = mock_account(2);
        let orch = AgentOrchestrator::new("gemini-1.5-pro", factory, settlement);
        let proposal = orch.process_intent("Please launch token named Qmoosa Token symbol QDOT");
        assert_eq!(proposal.action_type, ActionType::LaunchToken);
        assert!(proposal.requires_wallet_signature);
        assert_eq!(proposal.tx_proposal.unwrap().target, factory);
    }

    #[test]
    fn test_intent_x402_settle_proposal() {
        let factory = mock_account(1);
        let settlement = mock_account(2);
        let orch = AgentOrchestrator::new("gemini-1.5-pro", factory, settlement);
        let proposal = orch.process_intent("Settle x402 challenge for premium inference API");
        assert_eq!(proposal.action_type, ActionType::X402Settle);
        assert!(proposal.requires_wallet_signature);
        assert_eq!(proposal.tx_proposal.unwrap().target, settlement);
    }

    #[test]
    fn test_general_chat_no_signature_needed() {
        let factory = mock_account(1);
        let settlement = mock_account(2);
        let orch = AgentOrchestrator::new("gemini-1.5-pro", factory, settlement);
        let proposal = orch.process_intent("What is the current status of Polkadot Asset Hub?");
        assert_eq!(proposal.action_type, ActionType::GeneralChat);
        assert!(!proposal.requires_wallet_signature);
        assert!(proposal.tx_proposal.is_none());
    }
}
