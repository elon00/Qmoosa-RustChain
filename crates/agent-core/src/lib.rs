use qmoosa_primitives::AccountId32;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use thiserror::Error;

#[derive(Error, Debug, PartialEq, Eq)]
pub enum AgentError {
    #[error("Tool not found: {0}")]
    ToolNotFound(String),
    #[error("Invalid parameters: {0}")]
    InvalidParameters(String),
    #[error("Execution error: {0}")]
    ExecutionFailed(String),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ActionType {
    LaunchToken,
    X402Settle,
    NativeTransfer,
    AssetTransfer,
    XcmTransfer,
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

/// Metadata and JSON-Schema definition for an AI Agent Tool
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
}

/// Trait implemented by all Polkadot Agent Kit tools
pub trait AgentTool: Send + Sync {
    fn definition(&self) -> ToolDefinition;
    fn execute(&self, arguments: serde_json::Value) -> Result<serde_json::Value, AgentError>;
}

// --- Polkadot Agent Kit Standard Tools ---

pub struct PolkadotGetBalanceTool;

impl AgentTool for PolkadotGetBalanceTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "polkadot_get_balance".to_string(),
            description: "Check native DOT or asset balance for a Substrate SS58 address"
                .to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "account": { "type": "string", "description": "Substrate SS58 address (e.g. 5CwB8yg...)" },
                    "asset": { "type": "string", "description": "Token symbol (e.g. DOT or QDOT)", "default": "DOT" }
                },
                "required": ["account"]
            }),
        }
    }

    fn execute(&self, arguments: serde_json::Value) -> Result<serde_json::Value, AgentError> {
        let account_str = arguments["account"]
            .as_str()
            .ok_or_else(|| AgentError::InvalidParameters("Missing 'account' field".to_string()))?;

        let (acc, prefix) = AccountId32::from_ss58(account_str)
            .map_err(|e| AgentError::InvalidParameters(format!("Invalid SS58 address: {:?}", e)))?;

        let asset = arguments["asset"].as_str().unwrap_or("DOT");

        Ok(serde_json::json!({
            "account": acc.to_string(),
            "prefix": prefix,
            "asset": asset,
            "free_balance_plancks": 10_000_000_000u64, // 1 DOT
            "status": "active"
        }))
    }
}

pub struct PolkadotTransferNativeTool;

impl AgentTool for PolkadotTransferNativeTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "polkadot_transfer_native".to_string(),
            description: "Build a safe transaction proposal for transferring native DOT tokens"
                .to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "recipient": { "type": "string", "description": "Recipient Substrate SS58 address" },
                    "amount_plancks": { "type": "string", "description": "Amount in plancks (1 DOT = 10^10 plancks)" }
                },
                "required": ["recipient", "amount_plancks"]
            }),
        }
    }

    fn execute(&self, arguments: serde_json::Value) -> Result<serde_json::Value, AgentError> {
        let recipient_str = arguments["recipient"]
            .as_str()
            .ok_or_else(|| AgentError::InvalidParameters("Missing 'recipient'".to_string()))?;
        let amount_str = arguments["amount_plancks"]
            .as_str()
            .ok_or_else(|| AgentError::InvalidParameters("Missing 'amount_plancks'".to_string()))?;

        let (recipient, _) = AccountId32::from_ss58(recipient_str)
            .map_err(|e| AgentError::InvalidParameters(format!("Invalid SS58 address: {:?}", e)))?;
        let amount: u128 = amount_str.parse().map_err(|_| {
            AgentError::InvalidParameters("Invalid amount_plancks integer".to_string())
        })?;

        Ok(serde_json::json!({
            "action": "Balances.transfer_keep_alive",
            "recipient": recipient.to_string(),
            "amount_plancks": amount.to_string(),
            "requires_wallet_signature": true,
            "call_data_hex": format!("0x0403{}{:032x}", hex::encode(recipient.as_bytes()), amount)
        }))
    }
}

pub struct PolkadotTransferAssetTool;

impl AgentTool for PolkadotTransferAssetTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "polkadot_transfer_asset".to_string(),
            description: "Build a safe transaction proposal for transferring Asset Hub fungible tokens (e.g. QDOT)".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "asset_id": { "type": "number", "description": "Asset Hub integer asset ID (e.g. 1984 for QDOT)" },
                    "recipient": { "type": "string", "description": "Recipient Substrate SS58 address" },
                    "amount": { "type": "string", "description": "Token amount in base atomic units" }
                },
                "required": ["asset_id", "recipient", "amount"]
            }),
        }
    }

    fn execute(&self, arguments: serde_json::Value) -> Result<serde_json::Value, AgentError> {
        let asset_id = arguments["asset_id"]
            .as_u64()
            .ok_or_else(|| AgentError::InvalidParameters("Missing 'asset_id'".to_string()))?
            as u32;
        let recipient_str = arguments["recipient"]
            .as_str()
            .ok_or_else(|| AgentError::InvalidParameters("Missing 'recipient'".to_string()))?;
        let amount_str = arguments["amount"]
            .as_str()
            .ok_or_else(|| AgentError::InvalidParameters("Missing 'amount'".to_string()))?;

        let (recipient, _) = AccountId32::from_ss58(recipient_str)
            .map_err(|e| AgentError::InvalidParameters(format!("Invalid SS58 address: {:?}", e)))?;
        let amount: u128 = amount_str
            .parse()
            .map_err(|_| AgentError::InvalidParameters("Invalid amount integer".to_string()))?;

        Ok(serde_json::json!({
            "action": "Assets.transfer_keep_alive",
            "asset_id": asset_id,
            "recipient": recipient.to_string(),
            "amount": amount.to_string(),
            "requires_wallet_signature": true,
            "call_data_hex": format!("0x3201{:08x}{}{:032x}", asset_id, hex::encode(recipient.as_bytes()), amount)
        }))
    }
}

pub struct PolkadotXcmTransferTool;

impl AgentTool for PolkadotXcmTransferTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "polkadot_xcm_transfer".to_string(),
            description: "Build an XCM cross-consensus teleport or reserve transfer transaction"
                .to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "destination_para_id": { "type": "number", "description": "Target Parachain ID (e.g. 1000 for Asset Hub)" },
                    "recipient": { "type": "string", "description": "Recipient Substrate SS58 address on destination chain" },
                    "amount_plancks": { "type": "string", "description": "Amount in plancks to teleport" }
                },
                "required": ["destination_para_id", "recipient", "amount_plancks"]
            }),
        }
    }

    fn execute(&self, arguments: serde_json::Value) -> Result<serde_json::Value, AgentError> {
        let para_id = arguments["destination_para_id"].as_u64().ok_or_else(|| {
            AgentError::InvalidParameters("Missing 'destination_para_id'".to_string())
        })? as u32;
        let recipient_str = arguments["recipient"]
            .as_str()
            .ok_or_else(|| AgentError::InvalidParameters("Missing 'recipient'".to_string()))?;
        let amount_str = arguments["amount_plancks"]
            .as_str()
            .ok_or_else(|| AgentError::InvalidParameters("Missing 'amount_plancks'".to_string()))?;

        let (recipient, _) = AccountId32::from_ss58(recipient_str)
            .map_err(|e| AgentError::InvalidParameters(format!("Invalid SS58 address: {:?}", e)))?;
        let amount: u128 = amount_str.parse().map_err(|_| {
            AgentError::InvalidParameters("Invalid amount_plancks integer".to_string())
        })?;

        Ok(serde_json::json!({
            "action": "PolkadotXcm.limited_teleport_assets",
            "destination_para_id": para_id,
            "recipient": recipient.to_string(),
            "amount_plancks": amount.to_string(),
            "xcm_version": "V3",
            "requires_wallet_signature": true
        }))
    }
}

/// Central Polkadot Agent Kit registry providing tool discovery and execution
pub struct PolkadotAgentKit {
    tools: HashMap<String, Arc<dyn AgentTool>>,
}

impl PolkadotAgentKit {
    pub fn new() -> Self {
        let mut kit = Self {
            tools: HashMap::new(),
        };
        kit.register_tool(Arc::new(PolkadotGetBalanceTool));
        kit.register_tool(Arc::new(PolkadotTransferNativeTool));
        kit.register_tool(Arc::new(PolkadotTransferAssetTool));
        kit.register_tool(Arc::new(PolkadotXcmTransferTool));
        kit
    }

    pub fn register_tool(&mut self, tool: Arc<dyn AgentTool>) {
        let name = tool.definition().name;
        self.tools.insert(name, tool);
    }

    pub fn list_tools(&self) -> Vec<ToolDefinition> {
        self.tools.values().map(|t| t.definition()).collect()
    }

    pub fn call_tool(
        &self,
        name: &str,
        arguments: serde_json::Value,
    ) -> Result<serde_json::Value, AgentError> {
        let tool = self
            .tools
            .get(name)
            .ok_or_else(|| AgentError::ToolNotFound(name.to_string()))?;
        tool.execute(arguments)
    }

    /// Handles standard Model Context Protocol (MCP) JSON-RPC 2.0 requests
    pub fn handle_mcp_request(&self, request: &serde_json::Value) -> serde_json::Value {
        let id = request.get("id").cloned().unwrap_or(serde_json::json!(1));
        let method = match request.get("method").and_then(|m| m.as_str()) {
            Some(m) => m,
            None => {
                return serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "error": { "code": -32600, "message": "Invalid Request: missing method" }
                });
            }
        };

        match method {
            "initialize" => serde_json::json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": {
                    "protocolVersion": "2024-11-05",
                    "capabilities": { "tools": {} },
                    "serverInfo": {
                        "name": "qmoosa-polkadot-agent-kit",
                        "version": "0.1.0"
                    }
                }
            }),
            "tools/list" => serde_json::json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": { "tools": self.list_tools() }
            }),
            "tools/call" => {
                let params = match request.get("params") {
                    Some(p) => p,
                    None => {
                        return serde_json::json!({
                            "jsonrpc": "2.0",
                            "id": id,
                            "error": { "code": -32602, "message": "Invalid params" }
                        });
                    }
                };

                let tool_name = match params.get("name").and_then(|n| n.as_str()) {
                    Some(n) => n,
                    None => {
                        return serde_json::json!({
                            "jsonrpc": "2.0",
                            "id": id,
                            "error": { "code": -32602, "message": "Missing tool name" }
                        });
                    }
                };

                let arguments = params
                    .get("arguments")
                    .cloned()
                    .unwrap_or(serde_json::json!({}));

                match self.call_tool(tool_name, arguments) {
                    Ok(res) => serde_json::json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "result": {
                            "content": [{
                                "type": "text",
                                "text": serde_json::to_string_pretty(&res).unwrap_or_default()
                            }]
                        }
                    }),
                    Err(e) => serde_json::json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "error": { "code": -32000, "message": e.to_string() }
                    }),
                }
            }
            _ => serde_json::json!({
                "jsonrpc": "2.0",
                "id": id,
                "error": { "code": -32601, "message": format!("Method not found: {}", method) }
            }),
        }
    }
}

impl Default for PolkadotAgentKit {
    fn default() -> Self {
        Self::new()
    }
}

pub struct AgentOrchestrator {
    pub default_model: String,
    pub factory_account: AccountId32,
    pub x402_settlement_account: AccountId32,
    pub agent_kit: PolkadotAgentKit,
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
            agent_kit: PolkadotAgentKit::new(),
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
    fn test_agent_kit_list_tools() {
        let kit = PolkadotAgentKit::new();
        let tools = kit.list_tools();
        assert!(tools.len() >= 4);
        let names: Vec<String> = tools.into_iter().map(|t| t.name).collect();
        assert!(names.contains(&"polkadot_get_balance".to_string()));
        assert!(names.contains(&"polkadot_transfer_native".to_string()));
        assert!(names.contains(&"polkadot_transfer_asset".to_string()));
        assert!(names.contains(&"polkadot_xcm_transfer".to_string()));
    }

    #[test]
    fn test_agent_kit_transfer_native_tool() {
        let kit = PolkadotAgentKit::new();
        let user_addr = "5CwB8yg9VmeHX1CCYbzrNZWoj1uTLcVqGHgqBAasXetnQwnd";
        let res = kit
            .call_tool(
                "polkadot_transfer_native",
                serde_json::json!({
                    "recipient": user_addr,
                    "amount_plancks": "50000000000"
                }),
            )
            .unwrap();

        assert_eq!(res["action"], "Balances.transfer_keep_alive");
        assert_eq!(res["recipient"], user_addr);
        assert_eq!(res["amount_plancks"], "50000000000");
        assert!(res["requires_wallet_signature"].as_bool().unwrap());
    }

    #[test]
    fn test_agent_kit_transfer_asset_tool() {
        let kit = PolkadotAgentKit::new();
        let user_addr = "5CwB8yg9VmeHX1CCYbzrNZWoj1uTLcVqGHgqBAasXetnQwnd";
        let res = kit
            .call_tool(
                "polkadot_transfer_asset",
                serde_json::json!({
                    "asset_id": 1984,
                    "recipient": user_addr,
                    "amount": "1000000000000000000"
                }),
            )
            .unwrap();

        assert_eq!(res["action"], "Assets.transfer_keep_alive");
        assert_eq!(res["asset_id"], 1984);
        assert_eq!(res["recipient"], user_addr);
    }

    #[test]
    fn test_agent_kit_xcm_transfer_tool() {
        let kit = PolkadotAgentKit::new();
        let user_addr = "5CwB8yg9VmeHX1CCYbzrNZWoj1uTLcVqGHgqBAasXetnQwnd";
        let res = kit
            .call_tool(
                "polkadot_xcm_transfer",
                serde_json::json!({
                    "destination_para_id": 1000,
                    "recipient": user_addr,
                    "amount_plancks": "20000000000"
                }),
            )
            .unwrap();

        assert_eq!(res["action"], "PolkadotXcm.limited_teleport_assets");
        assert_eq!(res["destination_para_id"], 1000);
        assert_eq!(res["xcm_version"], "V3");
    }

    #[test]
    fn test_agent_kit_mcp_initialize_and_tools_list() {
        let kit = PolkadotAgentKit::new();

        // 1. MCP initialize
        let init_req = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize"
        });
        let init_res = kit.handle_mcp_request(&init_req);
        assert_eq!(
            init_res["result"]["serverInfo"]["name"],
            "qmoosa-polkadot-agent-kit"
        );

        // 2. MCP tools/list
        let list_req = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "tools/list"
        });
        let list_res = kit.handle_mcp_request(&list_req);
        let tools = list_res["result"]["tools"].as_array().unwrap();
        assert!(tools.len() >= 4);

        // 3. MCP tools/call
        let call_req = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 3,
            "method": "tools/call",
            "params": {
                "name": "polkadot_get_balance",
                "arguments": {
                    "account": "5CwB8yg9VmeHX1CCYbzrNZWoj1uTLcVqGHgqBAasXetnQwnd"
                }
            }
        });
        let call_res = kit.handle_mcp_request(&call_req);
        assert!(call_res["result"]["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("5CwB8yg"));
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
