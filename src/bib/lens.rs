use crate::bib::{
    constants::LENS_API,
    contracts::initialize_api,
    types::{LensTokens, Mention, TokensAlmacenados},
};
use dotenv::{from_filename, var};
use ethers::{
    providers::Middleware,
    signers::{LocalWallet, Signer},
    types::{transaction::eip2718::TypedTransaction, Bytes, Eip1559TransactionRequest},
    utils::hex,
};
use rand::{thread_rng, Rng};
use reqwest::Client;
use serde_json::{json, Value};
use std::{
    error::Error,
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

use super::contracts::{initialize_provider, initialize_wallet};

async fn refresh(
    client: Arc<Client>,
    refresh_tokens: &str,
    auth_tokens: &str,
) -> Result<LensTokens, Box<dyn Error + Send + Sync>> {
    let query = json!({
        "query": r#"
            mutation Refresh($request: RefreshRequest!) {
                refresh(request: $request) {
                     __typename
                        ... on AuthenticationTokens {
                    accessToken
                    refreshToken
                    idToken
                    }
                ... on ForbiddenError {
                    reason
                }
                }
            }
        "#,
        "variables": {
            "request": {
                "refreshToken": refresh_tokens.to_string()
            }
        }
    });

    from_filename(".env").ok();
    let server_key: String = var("SERVER_KEY").expect("SERVER_KEY not configured in .env");

    let response = client
        .post(LENS_API)
        .header("Authorization", format!("Bearer {}", auth_tokens))
        .header("x-api-key", server_key)
        .header("Content-Type", "application/json")
        .header("Origin", "https://npc-rust-engine.onrender.com")
        // .header("Origin", "http://localhost:3000")
        .json(&query)
        .send()
        .await?;

    if response.status().is_success() {
        let json: Value = response.json().await?;

        if let Some(authentication) = json["data"]["refresh"].as_object() {
            Ok(LensTokens {
                access_token: authentication
                    .get("accessToken")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string(),
                refresh_token: authentication
                    .get("refreshToken")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string(),
                id_token: authentication
                    .get("idToken")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string(),
            })
        } else {
            return Err("Unexpected Structure.".into());
        }
    } else {
        return Err(format!("Error: {}", response.status()).into());
    }
}

pub async fn authenticate(
    client: Arc<Client>,
    wallet: &LocalWallet,
    account_address: &str,
) -> Result<LensTokens, Box<dyn Error + Send + Sync>> {
    let mutation = json!({
        "query": r#"
        mutation Challenge($request: ChallengeRequest!) {
            challenge(request: $request) {
                __typename
                id
                text
            }
        }
    "#,
        "variables": {
            "request": {
                "accountOwner": {
                    "account": account_address.to_lowercase(),
                    "owner": format!("{:?}", wallet.address()).to_lowercase()
                }
            }
        }
    });

    from_filename(".env").ok();
    let server_key: String = var("SERVER_KEY").expect("SERVER_KEY not configured in .env");

    let res = client
        .post(LENS_API)
        .header("Content-Type", "application/json")
        .header("x-api-key", server_key)
        .header("Origin", "https://npc-rust-engine.onrender.com")
        // .header("Origin", "http://localhost:3000")
        .json(&mutation)
        .send()
        .await;

    match res {
        Ok(response) => {
            if response.status().is_success() {
                let json: Value = response.json().await?;
                if let Some(challenge) = json["data"]["challenge"].as_object() {
                    let text = challenge
                        .get("text")
                        .and_then(|v| v.as_str())
                        .unwrap_or_default();
                    let signature = wallet.sign_message(text).await?;

                    let authenticate_mutation = json!({
                        "query": r#"
                        mutation Authenticate($request: SignedAuthChallenge!) {
                            authenticate(request: $request) {
                                ... on AuthenticationTokens {
                                    accessToken
                                    refreshToken
                                    idToken
                                }
                                ... on WrongSignerError {
                                    reason
                                }
                                ... on ExpiredChallengeError {
                                    reason
                                }
                                ... on ForbiddenError {
                                    reason
                                }
                            }
                        }
                    "#,
                        "variables": {
                            "request": {
                                "id": challenge
                                    .get("id")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or_default(),
                                "signature": format!("0x{}", hex::encode(signature.to_vec())),
                            }
                        }
                    });

                    from_filename(".env").ok();
                    let server_key: String =
                        var("SERVER_KEY").expect("SERVER_KEY not configured in .env");

                    let response = client
                        .post(LENS_API)
                        .header("x-api-key", server_key)
                        .header("Content-Type", "application/json")
                        .header("Origin", "https://npc-rust-engine.onrender.com")
                        // .header("Origin", "http://localhost:3000")
                        .json(&authenticate_mutation)
                        .send()
                        .await?;

                    if response.status().is_success() {
                        let json: Value = response.json().await?;
                        if let Some(authentication) = json["data"]["authenticate"].as_object() {
                            return Ok(LensTokens {
                                access_token: authentication
                                    .get("accessToken")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or_default()
                                    .to_string(),
                                refresh_token: authentication
                                    .get("refreshToken")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or_default()
                                    .to_string(),
                                id_token: authentication
                                    .get("idToken")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or_default()
                                    .to_string(),
                            });
                        } else {
                            return Err("Authentication failed.".into());
                        }
                    } else {
                        return Err(format!("Error: {}", response.status()).into());
                    }
                } else {
                    return Err("Challenge response structure invalid.".into());
                }
            } else {
                return Err(format!("Error: {}", response.status()).into());
            }
        }
        Err(err) => {
            return Err(format!("Error: {}", err).into());
        }
    }
}

pub async fn handle_tokens(
    private_key: &str,
    account_address: &str,
    tokens: Option<TokensAlmacenados>,
) -> Result<TokensAlmacenados, Box<dyn Error + Send + Sync>> {
    let client = initialize_api();

    let wallet = match initialize_wallet(private_key) {
        Some(wallet) => wallet,
        None => {
            eprintln!("Wallet initialization failed. Skipping agent tokens.");
            return Err("Wallet initialization failed. Skipping agent tokens.".into());
        }
    };

    if let Some(saved) = tokens {
        let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
        let expiry: u64 = saved.expiry.try_into().unwrap();
        if now < (expiry - 3600) {
            return Ok(saved);
        } else {
            let new_tokens = refresh(
                client,
                &saved.tokens.refresh_token,
                &saved.tokens.access_token,
            )
            .await?;

            return Ok(TokensAlmacenados {
                expiry: (SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs() + 30 * 60) as i64,
                tokens: new_tokens,
            });
        }
    } else {
        let new_tokens = authenticate(client, &wallet, account_address).await?;

        return Ok(TokensAlmacenados {
            expiry: (SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs() + 30 * 60) as i64,
            tokens: new_tokens,
        });
    }
}

pub async fn make_publication(
    content: &str,
    private_key: &str,
    auth_tokens: &str,
) -> Result<String, Box<dyn Error + Send + Sync>> {
    let client = initialize_api();

    let wallet = match initialize_wallet(private_key) {
        Some(wallet) => wallet,
        None => {
            eprintln!("Wallet initialization failed. Skipping publication.");
            return Err("Wallet initialization failed. Skipping publication.".into());
        }
    };

    let request = json!({
        "contentUri": content,

    });

    let query = json!({
        "query": r#"
            mutation post($request: CreatePostRequest!) {
                post(request: $request) {
                    ... on PostResponse {
                        hash
                    }
                    ... on SponsoredTransactionRequest {
                        raw {
                            to
                            from
                            data
                            gasLimit
                            maxFeePerGas
                            maxPriorityFeePerGas
                            value
                            chainId
                        }
                        reason
                    }
                    ... on TransactionWillFail {
                        reason
                    }
                }
            }
        "#,
        "variables": {
                "request": request
        }
    });

    from_filename(".env").ok();
    let server_key: String = var("SERVER_KEY").expect("SERVER_KEY not configured in .env");

    let response = client
        .post(LENS_API)
        .header("x-api-key", server_key)
        .header("Authorization", format!("Bearer {}", auth_tokens))
        .header("Content-Type", "application/json")
        .header("Origin", "https://npc-rust-engine.onrender.com")
        // .header("Origin", "http://localhost:3000")
        .json(&query)
        .send()
        .await?;

    let json: Value = response.json().await?;
    if let Some(post_response) = json["data"]["post"].as_object() {
        if let Some(hash) = post_response.get("hash").and_then(|v| v.as_str()) {
            println!("Post Hash: {:?}", hash);
            return poll(hash, auth_tokens).await;
        }

        if let Some(raw) = post_response.get("raw").and_then(|v| v.as_object()) {
            let to = raw.get("to").and_then(|v| v.as_str()).unwrap_or_default();
            let from = raw.get("from").and_then(|v| v.as_str()).unwrap_or_default();
            let data = raw.get("data").and_then(|v| v.as_str()).unwrap_or_default();

            if to.is_empty() || from.is_empty() || data.is_empty() {
                return Err("Invalid transaction data: missing required fields.".into());
            }

            let gas_limit = raw
                .get("gasLimit")
                .and_then(|v| v.as_u64())
                .ok_or("Invalid gasLimit")?;
            let max_fee_per_gas = raw
                .get("maxFeePerGas")
                .and_then(|v| v.as_str())
                .ok_or("Invalid maxFeePerGas")?
                .parse::<u128>()?;
            let max_priority_fee_per_gas = raw
                .get("maxPriorityFeePerGas")
                .and_then(|v| v.as_str())
                .ok_or("Invalid maxPriorityFeePerGas")?
                .parse::<u128>()?;
            let value = raw
                .get("value")
                .and_then(|v| v.as_str())
                .ok_or("Invalid value")?
                .parse::<u128>()?;
            let chain_id = raw
                .get("chainId")
                .and_then(|v| v.as_u64())
                .ok_or("Invalid chainId")?;

            let provider = initialize_provider();
            let current_nonce = provider
                .get_transaction_count(wallet.address(), None)
                .await?;

            let tx = Eip1559TransactionRequest {
                to: Some(to.parse()?),
                from: Some(from.parse()?),
                gas: Some(gas_limit.into()),
                max_fee_per_gas: Some(max_fee_per_gas.into()),
                max_priority_fee_per_gas: Some(max_priority_fee_per_gas.into()),
                value: Some(value.into()),
                data: Some(data.parse()?),
                chain_id: Some(chain_id.into()),
                nonce: Some(current_nonce.into()),
                access_list: vec![].into(),
            };

            let typed_tx = TypedTransaction::Eip1559(tx);
            let signed_tx = wallet.sign_transaction(&typed_tx).await?;
            let signed_tx_bytes = typed_tx.rlp_signed(&signed_tx);

            let pending_tx = provider
                .send_raw_transaction(Bytes::from(signed_tx_bytes))
                .await?;
            return Ok(format!("Transaction sent: {}", pending_tx.tx_hash()));
        }

        if let Some(reason) = post_response.get("reason").and_then(|v| v.as_str()) {
            return Err(format!("Transaction failed: {}", reason).into());
        }
    }

    Err("Unexpected response format.".into())
}

async fn poll(hash: &str, auth_tokens: &str) -> Result<String, Box<dyn Error + Send + Sync>> {
    let client = initialize_api();
    let query = json!({
        "query": r#"
            query TransactionStatus($request: TransactionStatusRequest!) {
                transactionStatus(request: $request) {
                    ... on NotIndexedYetStatus {
                        reason
                        txHasMined
                    }
                    ... on PendingTransactionStatus {
                        blockTimestamp
                    }
                    ... on FinishedTransactionStatus {
                        blockTimestamp
                    }
                    ... on FailedTransactionStatus {
                        reason
                        blockTimestamp
                    }
                }
            }
        "#,
        "variables": {
            "request": {
                "txHash": hash
            }
        }
    });

    from_filename(".env").ok();
    let server_key: String = var("SERVER_KEY").expect("SERVER_KEY not configured in .env");

    let response = client
        .post(LENS_API)
        .header("Authorization", format!("Bearer {}", auth_tokens))
        .header("x-api-key", server_key)
        .header("Content-Type", "application/json")
        .header("Origin", "https://npc-rust-engine.onrender.com")
        // .header("Origin", "http://localhost:3000")
        .json(&query)
        .send()
        .await?;

    if response.status().is_success() {
        let json: Value = response.json().await?;
        if let Some(status) = json["data"]["transactionStatus"].as_object() {
            if let Some(reason) = status.get("reason").and_then(|v| v.as_str()) {
                return Ok(format!("Transaction failed: {}", reason));
            } else if let Some(timestamp) = status.get("blockTimestamp").and_then(|v| v.as_str()) {
                return Ok(format!("Transaction finished at: {}", timestamp));
            } else if let Some(tx_mined) = status.get("txHasMined").and_then(|v| v.as_bool()) {
                return Ok(format!(
                    "Transaction not indexed yet. Has mined: {}",
                    tx_mined
                ));
            }
        }
        Err("Unknown transaction status".into())
    } else {
        Err(format!("Error: {}", response.status()).into())
    }
}

pub async fn handle_lens_account(wallet: &str, username: bool) -> Result<String, Box<dyn Error>> {
    let client = initialize_api();
    let query = json!({
        "query": r#"
            query AccountsAvailable($request: AccountsAvailableRequest!) {
                accountsAvailable(request: $request) {
                    items {
                      ... on AccountOwned { 
                        account {
                            address
                            username {     
                                localName
                            }
                        }
                    }
                    ... on AccountManaged { 
                        account {
                            address
                            username {
                                localName
                            }
                        }
                      }
                    }
                }
            }
        "#,
        "variables": {
            "request": {
                "managedBy": wallet,
                "includeOwned": true
            }
        }
    });

    from_filename(".env").ok();
    let server_key: String = var("SERVER_KEY").expect("SERVER_KEY not configured in .env");

    let response = client
        .post(LENS_API)
        .header("x-api-key", server_key)
        .header("Content-Type", "application/json")
        .header("Origin", "https://npc-rust-engine.onrender.com")
        // .header("Origin", "http://localhost:3000")
        .json(&query)
        .send()
        .await?;

    if response.status().is_success() {
        let json: Value = response.json().await?;
        if let Some(items) = json["data"]["accountsAvailable"]["items"].as_array() {
            for item in items {
                if username {
                    if let Some(account_username) = item["account"]
                        .get("username")
                        .and_then(|username| username.get("localName"))
                        .and_then(|local_name| local_name.as_str())
                    {
                        return Ok(account_username.to_string());
                    }
                } else {
                    if let Some(account_address) = item["account"]
                        .get("address")
                        .and_then(|addr| addr.as_str())
                    {
                        return Ok(account_address.to_string());
                    }
                }
            }
        }
        return Err("No valid accounts found in the response.".into());
    } else {
        return Err(format!("Error: {}", response.status()).into());
    }
}

pub async fn make_comment(
    content: &str,
    private_key: &str,
    auth_tokens: &str,
    comment_id: &str,
) -> Result<String, Box<dyn Error + Send + Sync>> {
    let client = initialize_api();

    let wallet = match initialize_wallet(private_key) {
        Some(wallet) => wallet,
        None => {
            eprintln!("Wallet initialization failed. Skipping comment.");
            return Err("Wallet initialization failed. Skipping comment.".into());
        }
    };

    let query = json!({
        "query": r#"
            mutation post($request: CreatePostRequest!)   {
                post(request: $request) {
                    ... on PostResponse {
                        hash
                    }
                    ... on SponsoredTransactionRequest {
                        raw {
                            to
                            from
                            data
                            gasLimit
                            maxFeePerGas
                            maxPriorityFeePerGas
                            value
                            chainId
                        }
                        reason
                    }
                    ... on TransactionWillFail {
                        reason
                    }
                }
            }
        "#,
        "variables": {
            "request": {
                "contentUri": content,
                "commentOn": {
                    "post": comment_id
                }
            }
        }
    });

    from_filename(".env").ok();
    let server_key: String = var("SERVER_KEY").expect("SERVER_KEY not configured in .env");

    let response = client
        .post(LENS_API)
        .header("x-api-key", server_key)
        .header("Authorization", format!("Bearer {}", auth_tokens))
        .header("Content-Type", "application/json")
        .header("Origin", "https://npc-rust-engine.onrender.com")
        // .header("Origin", "http://localhost:3000")
        .json(&query)
        .send()
        .await?;

    let json: Value = response.json().await?;

    if let Some(post_response) = json["data"]["post"].as_object() {
        if let Some(hash) = post_response.get("hash").and_then(|v| v.as_str()) {
            println!("Comment Hash: {:?}", hash);
            return poll(hash, auth_tokens).await;
        }

        if let Some(raw) = post_response.get("raw").and_then(|v| v.as_object()) {
            let to = raw.get("to").and_then(|v| v.as_str()).unwrap_or_default();
            let from = raw.get("from").and_then(|v| v.as_str()).unwrap_or_default();
            let data = raw.get("data").and_then(|v| v.as_str()).unwrap_or_default();

            if to.is_empty() || from.is_empty() || data.is_empty() {
                return Err("Invalid transaction data: missing required fields.".into());
            }

            let gas_limit = raw
                .get("gasLimit")
                .and_then(|v| v.as_u64())
                .ok_or("Invalid gasLimit")?;
            let max_fee_per_gas = raw
                .get("maxFeePerGas")
                .and_then(|v| v.as_str())
                .ok_or("Invalid maxFeePerGas")?
                .parse::<u128>()?;
            let max_priority_fee_per_gas = raw
                .get("maxPriorityFeePerGas")
                .and_then(|v| v.as_str())
                .ok_or("Invalid maxPriorityFeePerGas")?
                .parse::<u128>()?;
            let value = raw
                .get("value")
                .and_then(|v| v.as_str())
                .ok_or("Invalid value")?
                .parse::<u128>()?;
            let chain_id = raw
                .get("chainId")
                .and_then(|v| v.as_u64())
                .ok_or("Invalid chainId")?;

            let provider = initialize_provider();
            let current_nonce = provider
                .get_transaction_count(wallet.address(), None)
                .await?;

            let tx = Eip1559TransactionRequest {
                to: Some(to.parse()?),
                from: Some(from.parse()?),
                gas: Some(gas_limit.into()),
                max_fee_per_gas: Some(max_fee_per_gas.into()),
                max_priority_fee_per_gas: Some(max_priority_fee_per_gas.into()),
                value: Some(value.into()),
                data: Some(data.parse()?),
                chain_id: Some(chain_id.into()),
                nonce: Some(current_nonce.into()),
                access_list: vec![].into(),
            };

            let typed_tx = TypedTransaction::Eip1559(tx);
            let signed_tx = wallet.sign_transaction(&typed_tx).await?;
            let signed_tx_bytes = typed_tx.rlp_signed(&signed_tx);

            let pending_tx = provider
                .send_raw_transaction(Bytes::from(signed_tx_bytes))
                .await?;
            return Ok(format!("Transaction sent: {}", pending_tx.tx_hash()));
        }

        if let Some(reason) = post_response.get("reason").and_then(|v| v.as_str()) {
            return Err(format!("Transaction failed: {}", reason).into());
        }
    }

    Err("Unexpected response format.".into())
}

pub async fn make_quote(
    content: &str,
    private_key: &str,
    auth_tokens: &str,
    quote_id: &str,
) -> Result<String, Box<dyn Error + Send + Sync>> {
    let client = initialize_api();

    let wallet = match initialize_wallet(private_key) {
        Some(wallet) => wallet,
        None => {
            eprintln!("Wallet initialization failed. Skipping quote.");
            return Err("Wallet initialization failed. Skipping quote.".into());
        }
    };

    let query = json!({
        "query": r#"
            mutation post($request: CreatePostRequest!)   {
                post(request: $request) {
                    ... on PostResponse {
                        hash
                    }
                    ... on SponsoredTransactionRequest {
                        raw {
                            to
                            from
                            data
                            gasLimit
                            maxFeePerGas
                            maxPriorityFeePerGas
                            value
                            chainId
                        }
                        reason
                    }
                    ... on TransactionWillFail {
                        reason
                    }
                }
            }
        "#,
        "variables": {
            "request": {
                "contentUri": content,
                "quoteOf": {
                    "post": quote_id
                }
            }
        }
    });

    from_filename(".env").ok();
    let server_key: String = var("SERVER_KEY").expect("SERVER_KEY not configured in .env");

    let response = client
        .post(LENS_API)
        .header("Authorization", format!("Bearer {}", auth_tokens))
        .header("x-api-key", server_key)
        .header("Content-Type", "application/json")
        .header("Origin", "https://npc-rust-engine.onrender.com")
        // .header("Origin", "http://localhost:3000")
        .json(&query)
        .send()
        .await?;

    let json: Value = response.json().await?;

    if let Some(post_response) = json["data"]["post"].as_object() {
        if let Some(hash) = post_response.get("hash").and_then(|v| v.as_str()) {
            println!("Quote Hash: {:?}", hash);
            return poll(hash, auth_tokens).await;
        }

        if let Some(raw) = post_response.get("raw").and_then(|v| v.as_object()) {
            let to = raw.get("to").and_then(|v| v.as_str()).unwrap_or_default();
            let from = raw.get("from").and_then(|v| v.as_str()).unwrap_or_default();
            let data = raw.get("data").and_then(|v| v.as_str()).unwrap_or_default();

            if to.is_empty() || from.is_empty() || data.is_empty() {
                return Err("Invalid transaction data: missing required fields.".into());
            }

            let gas_limit = raw
                .get("gasLimit")
                .and_then(|v| v.as_u64())
                .ok_or("Invalid gasLimit")?;
            let max_fee_per_gas = raw
                .get("maxFeePerGas")
                .and_then(|v| v.as_str())
                .ok_or("Invalid maxFeePerGas")?
                .parse::<u128>()?;
            let max_priority_fee_per_gas = raw
                .get("maxPriorityFeePerGas")
                .and_then(|v| v.as_str())
                .ok_or("Invalid maxPriorityFeePerGas")?
                .parse::<u128>()?;
            let value = raw
                .get("value")
                .and_then(|v| v.as_str())
                .ok_or("Invalid value")?
                .parse::<u128>()?;
            let chain_id = raw
                .get("chainId")
                .and_then(|v| v.as_u64())
                .ok_or("Invalid chainId")?;

            let provider = initialize_provider();
            let current_nonce = provider
                .get_transaction_count(wallet.address(), None)
                .await?;

            let tx = Eip1559TransactionRequest {
                to: Some(to.parse()?),
                from: Some(from.parse()?),
                gas: Some(gas_limit.into()),
                max_fee_per_gas: Some(max_fee_per_gas.into()),
                max_priority_fee_per_gas: Some(max_priority_fee_per_gas.into()),
                value: Some(value.into()),
                data: Some(data.parse()?),
                chain_id: Some(chain_id.into()),
                nonce: Some(current_nonce.into()),
                access_list: vec![].into(),
            };

            let typed_tx = TypedTransaction::Eip1559(tx);
            let signed_tx = wallet.sign_transaction(&typed_tx).await?;
            let signed_tx_bytes = typed_tx.rlp_signed(&signed_tx);

            let pending_tx = provider
                .send_raw_transaction(Bytes::from(signed_tx_bytes))
                .await?;
            return Ok(format!("Transaction sent: {}", pending_tx.tx_hash()));
        }

        if let Some(reason) = post_response.get("reason").and_then(|v| v.as_str()) {
            return Err(format!("Transaction failed: {}", reason).into());
        }
    }

    Err("Unexpected response format.".into())
}

pub async fn find_comment(
    auth_tokens: &str,
    account_address: &str,
) -> Result<(String, String), Box<dyn Error + Send + Sync>> {
    let client = initialize_api();
    let query = json!({
        "query": r#"
        query Posts($request: PostsRequest!) {
            posts(request: $request) {
                items {
                    ... on Post {
                        id
                        author {
                            __typename
                            owner
                            address
                            createdAt
                            username {
                                id
                                localName
                            }
                            operations {
                                isFollowedByMe
                            }
                        }
                        metadata {
                            __typename
                            ... on ArticleMetadata {
                                content
                            }
                            ... on StoryMetadata {
                                content
                            }
                            ... on TextOnlyMetadata {
                                content
                            }
                        }
                    }
                }
            }
        }
        "#,
        "variables": {
            "request": {
                "pageSize": "FIFTY",
                "filter": {
                    "authors": [account_address],
                    "metadata": {
                        "mainContentFocus": ["ARTICLE", "TEXT_ONLY", "STORY"]
                    }
                },
            }
        }
    });

    from_filename(".env").ok();
    let server_key: String = var("SERVER_KEY").expect("SERVER_KEY not configured in .env");

    let res = client
        .post(LENS_API)
        .header("Content-Type", "application/json")
        .header("Authorization", format!("Bearer {}", auth_tokens))
        .header("x-api-key", server_key)
        .header("Origin", "https://npc-rust-engine.onrender.com")
        .json(&query)
        .send()
        .await?;

    if res.status().is_success() {
        let json: Value = res.json().await?;

        if let Some(posts) = json["data"]["posts"]["items"].as_array() {
            if !posts.is_empty() {
                let mut rng = thread_rng();
                let mut encontrado = None;
                let mut indice_aleatorio: usize = 0;

                while encontrado.is_none() {
                    indice_aleatorio = rng.gen_range(0..posts.len());
                    if let Some(id_value) = posts[indice_aleatorio].get("id") {
                        if let Some(id) = id_value.as_str() {
                            encontrado = Some(id);
                        }
                    }
                }

                if let Some(id) = encontrado {
                    if let Some(contenido) = posts[indice_aleatorio]["metadata"]["content"].as_str()
                    {
                        return Ok((contenido.to_string(), id.to_string()));
                    } else {
                        return Err(
                            "El contenido no se encuentra o no es una cadena de texto.".into()
                        );
                    }
                } else {
                    return Err("ID no encontrado o no es una cadena de texto.".into());
                }
            } else {
                return Err("Items Empty".into());
            }
        } else {
            return Err("Error: Unexpected Structure for search posts".into());
        }
    } else {
        return Err(format!("Error: {}", res.status()).into());
    }
}

pub async fn make_like(
    auth_tokens: &str,
    gusta_on: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    let client = initialize_api();
    let consulta = json!({
        "query": r#"
            mutation AddReaction($request: AddReactionRequest!) {
                addReaction(request: $request)
            }
        "#,
        "variables": {
            "request": {
                "post": gusta_on,
                "reaction": "UPVOTE"
              }
        }
    });

    from_filename(".env").ok();
    let server_key: String = var("SERVER_KEY").expect("SERVER_KEY not configured in .env");

    let res = client
        .post(LENS_API)
        .header("Authorization", format!("Bearer {}", auth_tokens))
        .header("Content-Type", "application/json")
        .header("x-api-key", server_key)
        .header("Origin", "https://npc-rust-engine.onrender.com")
        .json(&consulta)
        .send()
        .await?;

    if res.status().is_success() {
        return Ok(gusta_on.to_string());
    } else {
        return Err(format!("Error: {}", res.status()).into());
    }
}

pub async fn make_mirror(
    auth_tokens: &str,
    mirror_id: &str,
) -> Result<String, Box<dyn Error + Send + Sync>> {
    let client = initialize_api();
    let query = json!({
        "query": r#"
            mutation repost($request: CreateRepostRequest!)   {
                repost(request: $request) {
                    ... on PostResponse {
                        hash
                    }
                    ... on SponsoredTransactionRequest {
                        raw {
                            to
                            from
                            data
                            gasLimit
                            maxFeePerGas
                            maxPriorityFeePerGas
                            value
                            chainId
                        }
                        reason
                    }
                    ... on TransactionWillFail {
                        reason
                    }
                }
            }
        "#,
        "variables": {
            "request": {
                "post": mirror_id,

            }
        }
    });

    from_filename(".env").ok();
    let server_key: String = var("SERVER_KEY").expect("SERVER_KEY not configured in .env");

    let response = client
        .post(LENS_API)
        .header("Authorization", format!("Bearer {}", auth_tokens))
        .header("x-api-key", server_key)
        .header("Content-Type", "application/json")
        .header("Origin", "https://npc-rust-engine.onrender.com")
        // .header("Origin", "http://localhost:3000")
        .json(&query)
        .send()
        .await?;

    if response.status().is_success() {
        return Ok(mirror_id.to_string());
    } else {
        return Err(format!("Error: {}", response.status()).into());
    }
}

pub async fn get_mentions(
    auth_tokens: &str,
    ultima_mencion: &str,
) -> Result<Vec<Mention>, Box<dyn Error + Send + Sync>> {
    let client = initialize_api();
    let query = json!({
       "query": r#"
    query Notifications($request: NotificationRequest!) {
        notifications(request: $request) {
            items {
                __typename
                ... on MentionNotification {
                    id
                    post {
                        id
                        metadata {
                            __typename
                            ... on ArticleMetadata {
                                content
                            }
                            ... on StoryMetadata {
                                content
                            }
                            ... on TextOnlyMetadata {
                                content
                            }
                            ... on ImageMetadata {
                                content
                            }
                        }
                    }
                }
            }
        }
    }
"#,

        "variables": {
            "request": {
                "filter": {
                    "notificationTypes": vec!["MENTIONED"]
                },

            }
        }
    });

    from_filename(".env").ok();
    let server_key: String = var("SERVER_KEY").expect("SERVER_KEY not configured in .env");

    let response = client
        .post(LENS_API)
        .header("Authorization", format!("Bearer {}", auth_tokens))
        .header("x-api-key", server_key)
        .header("Content-Type", "application/json")
        .header("Origin", "https://npc-rust-engine.onrender.com")
        .json(&query)
        .send()
        .await?;

    if response.status().is_success() {
        let json: Value = response.json().await?;

        println!("Notifications Json: {}", json);

        if let Some(mentions) = json["data"]["notifications"]["items"].as_array() {
            if !mentions.is_empty() {
                Ok(mentions
                    .iter()
                    .filter(|mention| mention["id"] != ultima_mencion)
                    .filter_map(|mention| {
                        let id = mention["id"].as_str()?.to_string();
                        let post_id = mention["post"]["id"].as_str()?.to_string();
                    
                        let metadata = &mention["post"]["metadata"];
                        let content = metadata["content"].as_str().unwrap_or("").to_string();
                    
                        Some(Mention { id, post_id, content })
                    })
                    .collect())
            } else {
                println!("No mentions");
                return Ok(Vec::new());
            }
        } else {
            eprintln!("Error` with notifications data");
            return Ok(Vec::new());
        }
    } else {
        return Err(format!("Error: {}", response.status()).into());
    }
}
