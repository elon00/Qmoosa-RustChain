use axum::{
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use qmoosa_agent_core::PolkadotAgentKit;
use qmoosa_pqc::{PqcProvider, PqcSignatureEnvelope};
use qmoosa_primitives::{AccountId32, SS58_PREFIX_SUBSTRATE};
use qmoosa_x402::{SubxtOnChainVerifier, X402BazaarGateway, X402Challenge, X402Proof};
use serde_json::json;
use std::sync::{Arc, Mutex};

struct AppState {
    x402_gateway: X402BazaarGateway,
    pqc_provider: PqcProvider,
    onchain_verifier: Arc<SubxtOnChainVerifier>,
    active_challenges: Mutex<std::collections::HashMap<String, X402Challenge>>,
    agent_kit: PolkadotAgentKit,
}

#[tokio::main]
async fn main() {
    let merchant_ss58 = "5CwB8yg9VmeHX1CCYbzrNZWoj1uTLcVqGHgqBAasXetnQwnd";
    let (merchant_account, _) =
        AccountId32::from_ss58(merchant_ss58).expect("Merchant address must be valid SS58");

    let gateway = X402BazaarGateway::new(
        merchant_account,
        500_000_000_000_000, // 0.05 DOT
        "DOT",
    );

    let rpc_url = std::env::var("POLKADOT_RPC_URL")
        .unwrap_or_else(|_| "wss://westend-asset-hub-rpc.polkadot.io:443".to_string());

    let mut verifier = SubxtOnChainVerifier::new(&rpc_url, "polkadot-asset-hub");

    // Attempt live Subxt WebSocket connection
    match SubxtOnChainVerifier::connect_live(&rpc_url).await {
        Ok(client) => {
            println!(
                "🌐 Successfully connected to live Subxt node at: {}",
                rpc_url
            );
            verifier = verifier.with_client(client);
        }
        Err(e) => {
            println!(
                "ℹ️ Live Subxt connection deferred ({}) - Verifier ready with SCALE decoding & cache",
                e
            );
        }
    }

    let verifier = Arc::new(verifier);

    let state = Arc::new(AppState {
        x402_gateway: gateway,
        pqc_provider: PqcProvider::new(),
        onchain_verifier: verifier,
        active_challenges: Mutex::new(std::collections::HashMap::new()),
        agent_kit: PolkadotAgentKit::new(),
    });

    let app = Router::new()
        .route("/health", get(health_handler))
        .route("/api/v1/alpha-model", get(protected_alpha_model_handler))
        .route("/api/pqc/sign", post(pqc_sign_handler))
        .route("/api/pqc/verify", post(pqc_verify_handler))
        .route("/api/v1/agent/tools", get(agent_tools_handler))
        .route("/mcp", post(mcp_handler))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080").await.unwrap();
    println!("🌌 Qmoosa-RustChain API Service listening on http://0.0.0.0:8080");
    println!("🤖 Polkadot Agent Kit & MCP Server active on /mcp");
    println!("🔑 Merchant Address: {}", merchant_ss58);
    axum::serve(listener, app).await.unwrap();
}

async fn health_handler(
    axum::extract::State(state): axum::extract::State<Arc<AppState>>,
) -> impl IntoResponse {
    (
        StatusCode::OK,
        Json(json!({
            "status": "online",
            "platform": "Qmoosa-RustChain",
            "architecture": "Polkadot-Native-PolkaVM",
            "pqc": "NIST-FIPS-204-ML-DSA-65",
            "x402": {
                "verifier": "SubxtOnChainVerifier",
                "subxt_version": "0.51.1",
                "rpc_url": state.onchain_verifier.rpc_url,
                "live_client_connected": state.onchain_verifier.has_client(),
                "network": state.onchain_verifier.expected_network
            },
            "agent_kit": {
                "engine": "Rust-native Polkadot Agent Kit (compatible/tooling layer)",
                "mcp_endpoint": "/mcp",
                "tools_count": state.agent_kit.list_tools().len()
            },
            "ss58_prefix": SS58_PREFIX_SUBSTRATE
        })),
    )
}

async fn protected_alpha_model_handler(
    axum::extract::State(state): axum::extract::State<Arc<AppState>>,
    headers: HeaderMap,
) -> impl IntoResponse {
    let payment_proof_hdr = headers.get("X-Payment-Proof");
    let challenge_hdr = headers.get("X-Payment-Challenge");
    let payer_hdr = headers.get("X-Payer-Address");
    let block_hash_hdr = headers.get("X-Block-Hash");

    if let (Some(proof_val), Some(challenge_val)) = (payment_proof_hdr, challenge_hdr) {
        let challenge_id = challenge_val.to_str().unwrap_or("").to_string();
        let tx_hash = proof_val.to_str().unwrap_or("").to_string();
        let payer_ss58 = payer_hdr
            .and_then(|h| h.to_str().ok())
            .unwrap_or("5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY");

        let payer_account = match AccountId32::from_ss58(payer_ss58) {
            Ok((acc, _)) => acc,
            Err(_) => {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(json!({ "error": "Invalid payer SS58 address" })),
                );
            }
        };

        // If an on-chain finalized block hash was provided and live client is available,
        // scan and register events directly from the live Substrate block
        if let Some(block_hdr) = block_hash_hdr {
            if let Ok(block_str) = block_hdr.to_str() {
                let clean_hex = block_str.strip_prefix("0x").unwrap_or(block_str);
                if let Ok(bytes) = hex::decode(clean_hex) {
                    if bytes.len() == 32 {
                        let mut block_hash_bytes = [0u8; 32];
                        block_hash_bytes.copy_from_slice(&bytes);
                        let _ = state
                            .onchain_verifier
                            .scan_and_register_block(block_hash_bytes, &tx_hash)
                            .await;
                    }
                }
            }
        }

        let proof = X402Proof {
            challenge_id: challenge_id.clone(),
            tx_hash,
            payer_address: payer_account,
        };

        let challenge_opt = {
            let guard = state.active_challenges.lock().unwrap();
            guard.get(&challenge_id).cloned()
        };

        if let Some(challenge) = challenge_opt {
            match state
                .x402_gateway
                .verify_payment(&challenge, &proof, &state.onchain_verifier)
            {
                Ok(details) => {
                    return (
                        StatusCode::OK,
                        Json(json!({
                            "status": "unlocked",
                            "resource": "Alpha Intelligence Model weights unlocked via Polkadot Settlement",
                            "payment_details": details
                        })),
                    );
                }
                Err(e) => {
                    return (
                        StatusCode::PAYMENT_REQUIRED,
                        Json(json!({
                            "status": 402,
                            "error": "Payment verification failed",
                            "details": e.to_string()
                        })),
                    );
                }
            }
        }
    }

    // Otherwise generate and store a new challenge
    let challenge = state.x402_gateway.generate_challenge(None, 15 * 60 * 1000);
    {
        let mut guard = state.active_challenges.lock().unwrap();
        guard.insert(challenge.challenge_id.clone(), challenge.clone());
    }

    (
        StatusCode::PAYMENT_REQUIRED,
        Json(json!({
            "status": 402,
            "message": "Payment Required via Polkadot Asset Hub",
            "challenge": challenge
        })),
    )
}

async fn pqc_sign_handler(
    axum::extract::State(state): axum::extract::State<Arc<AppState>>,
    Json(payload): Json<serde_json::Value>,
) -> impl IntoResponse {
    let raw = payload.to_string();
    let envelope = state.pqc_provider.sign_payload(&raw, 60000);
    (StatusCode::OK, Json(envelope))
}

#[derive(serde::Deserialize)]
struct VerifyRequest {
    payload: serde_json::Value,
    envelope: PqcSignatureEnvelope,
}

async fn pqc_verify_handler(Json(req): Json<VerifyRequest>) -> impl IntoResponse {
    let raw = req.payload.to_string();
    match PqcProvider::verify_envelope(&raw, &req.envelope, false) {
        Ok(_) => (
            StatusCode::OK,
            Json(json!({ "valid": true, "algorithm": "NIST-FIPS-204-ML-DSA-65" })),
        ),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(json!({ "valid": false, "error": e.to_string() })),
        ),
    }
}

async fn agent_tools_handler(
    axum::extract::State(state): axum::extract::State<Arc<AppState>>,
) -> impl IntoResponse {
    (StatusCode::OK, Json(state.agent_kit.list_tools()))
}

async fn mcp_handler(
    axum::extract::State(state): axum::extract::State<Arc<AppState>>,
    Json(request): Json<serde_json::Value>,
) -> impl IntoResponse {
    let response = state.agent_kit.handle_mcp_request(&request);
    (StatusCode::OK, Json(response))
}

#[cfg(test)]
mod tests {
    use super::*;
    use parity_scale_codec::Encode;
    use qmoosa_x402::SubstrateBalancesTransferEvent;

    fn create_test_state() -> Arc<AppState> {
        let (merchant_account, _) =
            AccountId32::from_ss58("5CwB8yg9VmeHX1CCYbzrNZWoj1uTLcVqGHgqBAasXetnQwnd")
                .expect("Valid SS58");

        let gateway = X402BazaarGateway::new(
            merchant_account,
            500_000_000_000_000, // 0.05 DOT
            "DOT",
        );

        let verifier = Arc::new(SubxtOnChainVerifier::new(
            "wss://westend-asset-hub-rpc.polkadot.io:443",
            "polkadot-asset-hub",
        ));

        Arc::new(AppState {
            x402_gateway: gateway,
            pqc_provider: PqcProvider::new(),
            onchain_verifier: verifier,
            active_challenges: Mutex::new(std::collections::HashMap::new()),
            agent_kit: PolkadotAgentKit::new(),
        })
    }

    #[tokio::test]
    async fn test_health_handler_shows_subxt_verifier() {
        let state = create_test_state();
        let resp = health_handler(axum::extract::State(state))
            .await
            .into_response();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_protected_alpha_model_unauthorized_flow() {
        let state = create_test_state();
        let headers = HeaderMap::new();

        let resp = protected_alpha_model_handler(axum::extract::State(state.clone()), headers)
            .await
            .into_response();

        assert_eq!(resp.status(), StatusCode::PAYMENT_REQUIRED);
        assert_eq!(state.active_challenges.lock().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn test_protected_alpha_model_authorized_with_subxt_verifier() {
        let state = create_test_state();

        // 1. Generate challenge
        let challenge = state.x402_gateway.generate_challenge(None, 60_000);
        state
            .active_challenges
            .lock()
            .unwrap()
            .insert(challenge.challenge_id.clone(), challenge.clone());

        // 2. Prepare payer and payment event
        let payer_ss58 = "5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY";
        let (payer_acc, _) = AccountId32::from_ss58(payer_ss58).unwrap();
        let tx_hash =
            "0x9999999999999999999999999999999999999999999999999999999999999999".to_string();

        let raw_scale_event = SubstrateBalancesTransferEvent {
            from: payer_acc,
            to: challenge.pay_to_address,
            amount: challenge.amount,
        }
        .encode();

        // Register in SubxtOnChainVerifier
        state
            .onchain_verifier
            .register_finalized_transfer(&tx_hash, 100, 105, &raw_scale_event, "DOT")
            .unwrap();

        // 3. Send headers with payment proof
        let mut headers = HeaderMap::new();
        headers.insert("X-Payment-Proof", tx_hash.parse().unwrap());
        headers.insert(
            "X-Payment-Challenge",
            challenge.challenge_id.parse().unwrap(),
        );
        headers.insert("X-Payer-Address", payer_ss58.parse().unwrap());

        let resp = protected_alpha_model_handler(axum::extract::State(state), headers)
            .await
            .into_response();

        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_mcp_handler_via_api() {
        let state = create_test_state();
        let mcp_req = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 100,
            "method": "initialize"
        });

        let resp = mcp_handler(axum::extract::State(state), Json(mcp_req))
            .await
            .into_response();

        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_agent_tools_handler() {
        let state = create_test_state();
        let resp = agent_tools_handler(axum::extract::State(state))
            .await
            .into_response();

        assert_eq!(resp.status(), StatusCode::OK);
    }
}
