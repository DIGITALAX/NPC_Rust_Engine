use crate::bib::{
    constants::API_LENS,
    contracts::{inicializar_api, inicializar_billetera, inicializar_proveedor},
    types::{LensTokens, Mention, TokensAlmacenados},
};
use ethers::{
    providers::Middleware,
    signers::{LocalWallet, Signer},
    types::{transaction::eip2718::TypedTransaction, Bytes, Eip1559TransactionRequest},
    utils::hex,
};
use futures::future::join_all;
use rand::Rng;
use reqwest::Client;
use serde_json::{json, Value};
use std::{
    error::Error,
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

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

    let response = client
        .post(API_LENS)
        .header("Authorization", format!("Bearer {}", auth_tokens))
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

    let res = client
        .post(API_LENS)
        .header("Content-Type", "application/json")
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

                    let response = client
                        .post(API_LENS)
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
    let client = inicializar_api();

    let wallet = inicializar_billetera(private_key);

    if let Some(saved) = tokens {
        let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
        let expiry: u64 = saved.expira_en.try_into().unwrap();
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
                expira_en: (SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs() + 30 * 60)
                    as i64,
                tokens: new_tokens,
            });
        }
    } else {
        let new_tokens = authenticate(client, &wallet, account_address).await?;

        return Ok(TokensAlmacenados {
            expira_en: (SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs() + 30 * 60) as i64,
            tokens: new_tokens,
        });
    }
}

pub async fn make_publication(
    content: &str,
    private_key: &str,
    auth_tokens: &str,
) -> Result<String, Box<dyn Error + Send + Sync>> {
    let client = inicializar_api();

    let wallet = inicializar_billetera(private_key);

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
                "request":  {
                    "contentUri": content,

                }
        }
    });

    let response = client
        .post(API_LENS)
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

            let provider = inicializar_proveedor();
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
    let client = inicializar_api();
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

    let response = client
        .post(API_LENS)
        .header("Authorization", format!("Bearer {}", auth_tokens))
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
    let client = inicializar_api();
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

    let response = client
        .post(API_LENS)
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

pub async fn search_posts(
    wallet: &str,
    search_query: &str,
) -> Result<(Vec<Value>, Vec<String>), Box<dyn Error + Send + Sync>> {
    let client = inicializar_api();

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
                    "searchQuery": search_query,
                    "metadata": {
                        "mainContentFocus": ["ARTICLE", "TEXT_ONLY", "STORY"]
                    }
                },
            }
        }
    });

    let res = client
        .post(API_LENS)
        .header("Content-Type", "application/json")
        .header("Origin", "https://npc-rust-engine.onrender.com")
        .json(&query)
        .send()
        .await?;

    if res.status().is_success() {
        let json: Value = res.json().await?;

        if let Some(posts) = json["data"]["posts"]["items"].as_array() {
            let filtered_posts: Vec<Value> = posts
                .iter()
                .filter(|post| {
                    if let Some(author) = post["author"].as_object() {
                        if let Some(owner) = author["owner"].as_str() {
                            return owner.to_lowercase() != wallet.to_string().to_lowercase();
                        }
                    }
                    false
                })
                .take(10)
                .cloned()
                .collect();

            let filtered_profiles: Vec<String> = posts
                .iter()
                .filter(|post| {
                    if let Some(author) = post["author"].as_object() {
                        if let Some(operations) = author["operations"].as_object() {
                            if let Some(following) = operations["isFollowedByMe"].as_bool() {
                                if !following {
                                    return true;
                                }
                            }
                        }
                    }
                    false
                })
                .filter_map(|post| post["author"]["address"].as_str().map(|s| s.to_string()))
                .take(10)
                .collect();
            return Ok((filtered_posts, filtered_profiles));
        } else {
            return Err("Error: Unexpected Structure for search posts".into());
        }
    } else {
        return Err(format!("Error: {}", res.status()).into());
    }
}

pub async fn follow_profiles(
    profiles: Vec<String>,
    auth_tokens: &str,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let client = inicializar_api();

    let follow_futures = profiles.into_iter().map(|profile| {
        let client = client.clone();
        let auth_tokens = auth_tokens.to_string();

        async move {
            let query = json!({
                "query": r#"
                    mutation Follow($request: FollowRequest!) {
                        follow(request: $request) {
                            ... on FollowResponse {
                                hash
                            }
                        }
                    }
                "#,
                "variables": {
                    "request": {
                        "account": profile
                    }
                }
            });

            let response = client
                .post(API_LENS)
                .header("Authorization", format!("Bearer {}", auth_tokens))
                .header("Content-Type", "application/json")
                .header("Origin", "https://npc-rust-engine.onrender.com")
                .json(&query)
                .send()
                .await?;

            if response.status().is_success() {
                let json: Value = response.json().await?;

                if let Some(follow_response) = json["data"]["follow"].as_object() {
                    if let Some(hash) = follow_response.get("hash").and_then(|v| v.as_str()) {
                        println!("Follow Hash for {}: {:?}", profile, hash);
                        let _ = poll(hash, &auth_tokens).await;
                    }
                } else {
                    println!("Unexpected structure for profile: {}", profile);
                }
            } else {
                println!("Error following profile {}: {}", profile, response.status());
            }

            Ok::<(), Box<dyn Error + Send + Sync>>(())
        }
    });

    let results: Vec<_> = join_all(follow_futures).await;

    for result in results {
        if let Err(e) = result {
            println!("Error with following: {:?}", e);
        }
    }

    Ok(())
}

pub async fn make_comment(
    content: &str,
    private_key: &str,
    auth_tokens: &str,
    comment_id: &str,
) -> Result<String, Box<dyn Error + Send + Sync>> {
    let client = inicializar_api();

    let wallet = inicializar_billetera(private_key);

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

    let response = client
        .post(API_LENS)
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

            let provider = inicializar_proveedor();
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
    let client = inicializar_api();

    let wallet = inicializar_billetera(private_key);

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

    let response = client
        .post(API_LENS)
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

            let provider = inicializar_proveedor();
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

pub async fn feed_info(feed: &str) -> Result<(String, String), Box<dyn Error + Send + Sync>> {
    let client = inicializar_api();

    let query = json!({
        "query": r#"
        query Feed($request: FeedRequest!) {
            feed(request: $request) {
                metadata {
                    title 
                    description
                }
            }
        }
        "#,
        "variables": {
            "request": {
                "filter": {
                    "feed": feed,
                },
            }
        }
    });

    let res = client
        .post(API_LENS)
        .header("Content-Type", "application/json")
        .header("Origin", "https://npc-rust-engine.onrender.com")
        .json(&query)
        .send()
        .await?;

    if res.status().is_success() {
        let json: Value = res.json().await?;

        if let Some(metadata) = json["data"]["feed"]["metadata"].as_object() {
            let mut description = String::from("");
            let mut title = String::from("");

            if let Some(des) = metadata["description"].as_str() {
                description = des.to_string();
            }
            if let Some(tit) = metadata["title"].as_str() {
                title = tit.to_string();
            }

            return Ok((title, description));
        } else {
            return Err("Error: Unexpected Structure for Feed Info".into());
        }
    } else {
        return Err(format!("Error: {}", res.status()).into());
    }
}

pub async fn find_comment(
    account_address: &str,
) -> Result<(String, String), Box<dyn Error + Send + Sync>> {
    let client = inicializar_api();
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

    let res = client
        .post(API_LENS)
        .header("Content-Type", "application/json")
        .header("Origin", "https://npc-rust-engine.onrender.com")
        .json(&query)
        .send()
        .await?;

    if res.status().is_success() {
        let json: Value = res.json().await?;

        if let Some(posts) = json["data"]["posts"]["items"].as_array() {
            if !posts.is_empty() {
                let mut rng = rand::thread_rng();
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

pub async fn make_like(gusta_on: &str) -> Result<String, Box<dyn std::error::Error>> {
    let client = inicializar_api();
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

    let res = client
        .post(API_LENS)
        .header("Content-Type", "application/json")
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
    let client = inicializar_api();
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

    let response = client
        .post(API_LENS)
        .header("Authorization", format!("Bearer {}", auth_tokens))
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
    let client = inicializar_api();
    let query = json!({
        "query": r#"
            mutation Notifications($request: NotificationRequest!)   {
                notifications(request: $request) {
                   ... on MentionNotification {
                    __typename
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
        "#,
        "variables": {
            "request": {
                "filter": {
                    "notificationTypes": vec!["MENTIONED"]
                },

            }
        }
    });

    let response = client
        .post(API_LENS)
        .header("Authorization", format!("Bearer {}", auth_tokens))
        .header("Content-Type", "application/json")
        .header("Origin", "https://npc-rust-engine.onrender.com")
        .json(&query)
        .send()
        .await?;

    if response.status().is_success() {
        let json: Value = response.json().await?;

        if let Some(mentions) = json["data"]["notifications"]["items"].as_array() {
            if !mentions.is_empty() {
                Ok(mentions
                    .iter()
                    .filter(|mention| mention["id"] != ultima_mencion)
                    .map(|mention| Mention {
                        id: mention["id"].to_string(),
                        content: mention["post"]["content"].to_string(),
                        post_id: mention["post"]["id"].to_string(),
                    })
                    .collect())
            } else {
                return Err("Error: Unexpected Structure for notifications".into());
            }
        } else {
            return Err("Error: with notifications data".into());
        }
    } else {
        return Err(format!("Error: {}", response.status()).into());
    }
}
