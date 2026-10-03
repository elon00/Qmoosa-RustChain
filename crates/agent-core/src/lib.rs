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
    pub target: String,
    pub value_dot: String,
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
}

impl AgentOrchestrator {
    pub fn new(default_model: &str) -> Self {
        Self {
            default_model: default_model.to_string(),
        }
    }

    pub fn process_intent(&self, user_prompt: &str) -> AgentActionProposal {
        let lower = user_prompt.to_lowercase();
        let proposal_id = format!("prop_{}", chrono::Utc::now().timestamp_millis());

        if lower.contains("launch token") || lower.contains("create token") {
            AgentActionProposal {
                proposal_id,
                action_type: ActionType::LaunchToken,
                summary: "Deploying new Polkadot Native dynamic uncapped token via PolkaVM".to_string(),
                requires_wallet_signature: true,
                tx_proposal: Some(TransactionProposal {
                    target: "TokenFactoryAddress".to_string(),
                    value_dot: "0.0".to_string(),
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
                    target: "X402SettlementAddress".to_string(),
                    value_dot: "0.05".to_string(),
                    call_data_hex: "0xsettleDot".to_string(),
                }),
            }
        } else {
            AgentActionProposal {
                proposal_id,
                action_type: ActionType::GeneralChat,
                summary: format!("Processed query with {}: '{}'", self.default_model, user_prompt),
                requires_wallet_signature: false,
                tx_proposal: None,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_intent_launch_token_proposal() {
        let orch = AgentOrchestrator::new("gemini-1.5-pro");
        let proposal = orch.process_intent("Please launch token named Qmoosa Token symbol QDOT");
        assert_eq!(proposal.action_type, ActionType::LaunchToken);
        assert!(proposal.requires_wallet_signature);
        assert!(proposal.tx_proposal.is_some());
    }

    #[test]
    fn test_intent_x402_settle_proposal() {
        let orch = AgentOrchestrator::new("gemini-1.5-pro");
        let proposal = orch.process_intent("Settle x402 challenge for premium inference API");
        assert_eq!(proposal.action_type, ActionType::X402Settle);
        assert!(proposal.requires_wallet_signature);
        assert_eq!(proposal.tx_proposal.unwrap().value_dot, "0.05");
    }

    #[test]
    fn test_general_chat_no_signature_needed() {
        let orch = AgentOrchestrator::new("claude-3.5-sonnet");
        let proposal = orch.process_intent("What is PolkaVM?");
        assert_eq!(proposal.action_type, ActionType::GeneralChat);
        assert!(!proposal.requires_wallet_signature);
        assert!(proposal.tx_proposal.is_none());
    }
}
