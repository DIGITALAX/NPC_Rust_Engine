use crate::bib::{
    constants::{API_LENS, AUTOGRAPH_DATA, LENS_HUB_PROXY},
    types::TokensAlmacenados,
    utils::{autenticar, refrescar},
};

use dotenv::dotenv;
use ethers::{
    abi::{Abi, Address},
    addressbook::Chain,
    contract::Contract,
    middleware::SignerMiddleware,
    providers::{Http, Provider, ProviderExt},
    signers::{LocalWallet, Signer},
    types::{
        transaction::eip712::{EIP712Domain, Eip712DomainType, TypedData},
        U256,
    },
};
use rand::Rng;
use reqwest::Client;
use serde_json::{from_str, json};
use std::{
    collections::BTreeMap,
    env::var,
    error::Error,
    sync::{Arc, OnceLock},
    time::{SystemTime, UNIX_EPOCH},
};

static LENS_HUB_PROXY_CONTRATO: OnceLock<
    Arc<Contract<SignerMiddleware<Arc<Provider<Http>>, LocalWallet>>>,
> = OnceLock::new();
static AUTOGRAPH_DATA_CONTRATO: OnceLock<
    Arc<Contract<SignerMiddleware<Arc<Provider<Http>>, LocalWallet>>>,
> = OnceLock::new();
static PROVEEDOR: OnceLock<Arc<Provider<Http>>> = OnceLock::new();
static CLIENTE_LENS: OnceLock<Arc<Client>> = OnceLock::new();
static BILLETERA: OnceLock<LocalWallet> = OnceLock::new();

pub fn inicializar_proveedor() -> Arc<Provider<Http>> {
    PROVEEDOR
        .get_or_init(|| {
            dotenv().ok();
            let proveedor_url = format!(
                "https://polygon-mainnet.g.alchemy.com/v2/{}",
                var("ALCHEMY_API_KEY").expect("ALCHEMY_API_KEY not found")
            );

            let mut proveedor =
                Provider::<Http>::try_from(&proveedor_url).expect("Error al crear proveedor");

            proveedor.set_chain(Chain::Polygon);

            Arc::new(proveedor)
        })
        .clone()
}

pub fn inicializar_api() -> Arc<Client> {
    CLIENTE_LENS.get_or_init(|| Arc::new(Client::new())).clone()
}

pub fn inicializar_billetera(clave_privada: &str) -> &'static LocalWallet {
    BILLETERA.get_or_init(|| {
        let billetera = match var(clave_privada) {
            Ok(key) => match key.parse::<LocalWallet>() {
                Ok(wallet) => wallet.with_chain_id(Chain::Polygon),
                Err(e) => panic!("Error al parsear la clave privada: {:?}", e),
            },
            Err(e) => panic!("PRIVATE_KEY not found: {:?}", e),
        };
        billetera
    })
}

pub async fn obtener_o_refrescar_tokens(
    clave_privada: &str,
    perfil_id: U256,
    tokens: Option<TokensAlmacenados>,
) -> Result<TokensAlmacenados, Box<dyn std::error::Error>> {
    let cliente = inicializar_api();
    let billetera = inicializar_billetera(&clave_privada);

    if let Some(almacenados) = tokens {
        let ahora = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();

        if ahora < almacenados.expira_en.try_into().unwrap() {
            return Ok(almacenados);
        } else {
            let tokens_nuevos = refrescar(
                cliente,
                &almacenados.tokens.refresh_token,
                &almacenados.tokens.access_token,
            )
            .await?;

            return Ok(TokensAlmacenados {
                expira_en: (SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs() + 30 * 60)
                    as i64,
                tokens: tokens_nuevos,
            });
        }
    } else {
        let tokens_nuevos = autenticar(cliente, &billetera, &format!("0x0{:x}", perfil_id)).await?;
        return Ok(TokensAlmacenados {
            expira_en: (SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs() + 30 * 60) as i64,
            tokens: tokens_nuevos,
        });
    }
}

pub async fn coger_comentario(
    perfil_id: &str,
) -> Result<(String, U256, U256, String), Box<dyn Error + Send + Sync>> {
    let cliente = inicializar_api();
    let consulta = json!({
        "query": r#"
            query Publications($request: PublicationsRequest!) {
                publications(request: $request) {
                    items {
                        ... on Post {
                            id
                            metadata {
                                ... on TextOnlyMetadataV3 {
                                    content
                                    rawURI
                                }
                                ... on ImageMetadataV3 {
                                    content
                                    rawURI
                                }
                                ... on VideoMetadataV3 {
                                    content
                                    rawURI
                                }
                            }
                        }
                    }
                }
            }
        "#,
        "variables": {
            "request": {
                "where": {
                    "from": [perfil_id]
                }
            }
        }
    });
    let respuesta = cliente
        .post(API_LENS)
        .header("Content-Type", "application/json")
        .json(&consulta)
        .send()
        .await?;

    if respuesta.status().is_success() {
        let json: serde_json::Value = respuesta.json().await?;

        if let Some(items) = json["data"]["publications"]["items"].as_array() {
            if !items.is_empty() {
                let mut rng = rand::thread_rng();
                let mut encontrado = None;
                let mut indice_aleatorio: usize = 0;

                while encontrado.is_none() {
                    indice_aleatorio = rng.gen_range(0..items.len());
                    if let Some(id_value) = items[indice_aleatorio].get("id") {
                        if let Some(id) = id_value.as_str() {
                            encontrado = Some(id);
                        }
                    }
                }

                if let Some(id) = encontrado {
                    if let Some(hex_str) = id.split('-').nth(0) {
                        let comentario_perfil = U256::from_str_radix(&hex_str[2..], 16)?;

                        if let Some(hex_str) = id.split('-').nth(1) {
                            let comentario_pub = U256::from_str_radix(&hex_str[2..], 16)?;

                            if let Some(contenido) =
                                items[indice_aleatorio]["metadata"]["content"].as_str()
                            {
                                if let Some(metadata_uri) =
                                    items[indice_aleatorio]["metadata"]["rawURI"].as_str()
                                {
                                    return Ok((
                                        contenido.to_string(),
                                        comentario_perfil,
                                        comentario_pub,
                                        metadata_uri.to_string(),
                                    ));
                                } else {
                                    return Err(
                                        "El metadata no se encuentra o no es una cadena de texto."
                                            .into(),
                                    );
                                }
                            } else {
                                return Err(
                                    "El contenido no se encuentra o no es una cadena de texto."
                                        .into(),
                                );
                            }
                        } else {
                            return Err("Error con el Hex".into());
                        }
                    } else {
                        return Err("Error con el Hex".into());
                    }
                } else {
                    return Err("ID no encontrado o no es una cadena de texto.".into());
                }
            } else {
                return Ok(("".to_string(), U256::from(0), U256::from(0), "".to_string()));
            }
        } else {
            return Ok(("".to_string(), U256::from(0), U256::from(0), "".to_string()));
        }
    } else {
        return Err(format!("Error: {}", respuesta.status()).into());
    }
}


pub fn inicializar_contrato(
    clave_privada: &str,
) -> (
    Arc<Contract<SignerMiddleware<Arc<Provider<Http>>, LocalWallet>>>,
    Arc<Contract<SignerMiddleware<Arc<Provider<Http>>, LocalWallet>>>,
) {
    dotenv().ok();
    let proveedor = inicializar_proveedor();
    let billetera = inicializar_billetera(clave_privada);
    let cliente = Arc::new(SignerMiddleware::new(proveedor.clone(), billetera.clone()));

    let lens_hub_contrato = LENS_HUB_PROXY_CONTRATO
        .get_or_init(|| {
            let direccion = LENS_HUB_PROXY
                .parse::<Address>()
                .expect("Error al parsear LENS_HUB_PROXY");

            let abi: Abi = from_str(include_str!("./../../abis/LensHubProxy.json"))
                .expect("Error al cargar LensHubProxy ABI");

            Arc::new(Contract::new(direccion, abi, cliente.clone()))
        })
        .clone();

    let autograph_data_contrato = AUTOGRAPH_DATA_CONTRATO
        .get_or_init(|| {
            let direccion = AUTOGRAPH_DATA
                .parse::<Address>()
                .expect("Error al parsear AUTOGRAPH_DATA");

            let abi: Abi = from_str(include_str!("./../../abis/AutographData.json"))
                .expect("Error al cargar AutographData ABI");

            Arc::new(Contract::new(direccion, abi, cliente.clone()))
        })
        .clone();

    (lens_hub_contrato, autograph_data_contrato)
}

pub async fn hacer_comentario(
    clave_privada: &str,
    comentario_on: &str,
    contenido_uri: String,
    token_autorizado: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    let cliente = inicializar_api();
    let billetera = inicializar_billetera(&clave_privada);

    let consulta = json!({
        "query": r#"
            mutation CreateOnchainCommentTypedData($request: OnchainCommentRequest!) {
                createOnchainCommentTypedData(request: $request) {
      id
      expiresAt
      typedData {
        types {
          Comment {
            name
            type
          }
        }
        domain {
          name
          chainId
          version
          verifyingContract
        }
        value {
          nonce
          deadline
          profileId
          contentURI
          pointedProfileId
          pointedPubId
          referrerProfileIds
          referrerPubIds
          referenceModuleData
          actionModules
          actionModulesInitDatas
          referenceModule
          referenceModuleInitData
        }
      }
    }
            }
        "#,
        "variables": {
            "request": {
                "commentOn":comentario_on,
                "contentURI":contenido_uri,
                "openActionModules": [{
                    "collectOpenAction": {
                        "simpleCollectOpenAction": {
                            "followerOnly": false
                        }
                    }
                }]
            }
        }
    });

    let respuesta = cliente
        .post(API_LENS)
        .header("Authorization", format!("Bearer {}", token_autorizado))
        .header("Content-Type", "application/json")
        .json(&consulta)
        .send()
        .await?;

    if respuesta.status().is_success() {
        let json: serde_json::Value = respuesta.json().await?;
        if let Some(datos) = json["data"]["createOnchainCommentTypedData"].as_object() {
            let datos_escritos = datos.get("typedData").and_then(|v| v.as_object()).unwrap();

            let domain = serde_json::from_value::<EIP712Domain>(
                datos_escritos.get("domain").cloned().unwrap(),
            )?;
            let types = serde_json::from_value::<BTreeMap<String, Vec<Eip712DomainType>>>(
                datos_escritos.get("types").cloned().unwrap(),
            )?;
            let value = serde_json::from_value::<BTreeMap<String, serde_json::Value>>(
                datos_escritos.get("value").cloned().unwrap(),
            )?;

            let firma = billetera
                .sign_typed_data(&TypedData {
                    domain,
                    types,
                    primary_type: "Comment".to_string(),
                    message: value,
                })
                .await?;

            return Ok(propogar(
                datos.get("id").and_then(|v| v.as_str()).unwrap_or_default(),
                &firma.to_string(),
                token_autorizado,
            )
            .await?);
        } else {
            return Err("Estructura de respuesta inesperada Comentario.".into());
        }
    } else {
        return Err(format!("Error: {}", respuesta.status()).into());
    }
}

pub async fn hacer_cita(
    clave_privada: &str,
    cita_on: &str,
    contenido_uri: String,
    token_autorizado: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    let cliente = inicializar_api();
    let billetera = inicializar_billetera(&clave_privada);

    let consulta = json!({
        "query": r#"
            mutation CreateOnchainQuoteTypedData($request: OnchainQuoteRequest!) {
                createOnchainQuoteTypedData(request: $request) {
      id
      expiresAt
      typedData {
        types {
          Quote {
            name
            type
          }
        }
        domain {
          name
          chainId
          version
          verifyingContract
        }
        value {
          nonce
          deadline
          profileId
          contentURI
          pointedProfileId
          pointedPubId
          referrerProfileIds
          referrerPubIds
          referenceModuleData
          actionModules
          actionModulesInitDatas
          referenceModule
          referenceModuleInitData
        }
      }
    }
            }
        "#,
        "variables": {
            "request": {
                "quoteOn": cita_on,
                "contentURI":contenido_uri,
                "openActionModules": [{
                    "collectOpenAction": {
                        "simpleCollectOpenAction": {
                            "followerOnly": false
                        }
                    }
                }]
            }
        }
    });

    let respuesta = cliente
        .post(API_LENS)
        .header("Authorization", format!("Bearer {}", token_autorizado))
        .header("Content-Type", "application/json")
        .json(&consulta)
        .send()
        .await?;

    if respuesta.status().is_success() {
        let json: serde_json::Value = respuesta.json().await?;
        if let Some(datos) = json["data"]["createOnchainQuoteTypedData"].as_object() {
            let datos_escritos = datos.get("typedData").and_then(|v| v.as_object()).unwrap();
            let domain = serde_json::from_value::<EIP712Domain>(
                datos_escritos.get("domain").cloned().unwrap(),
            )?;
            let types = serde_json::from_value::<BTreeMap<String, Vec<Eip712DomainType>>>(
                datos_escritos.get("types").cloned().unwrap(),
            )?;
            let value = serde_json::from_value::<BTreeMap<String, serde_json::Value>>(
                datos_escritos.get("value").cloned().unwrap(),
            )?;

            let firma = billetera
                .sign_typed_data(&TypedData {
                    domain,
                    types,
                    primary_type: "Quote".to_string(),
                    message: value,
                })
                .await?;

            return Ok(propogar(
                datos.get("id").and_then(|v| v.as_str()).unwrap_or_default(),
                &firma.to_string(),
                token_autorizado,
            )
            .await?);
        } else {
            return Err("Estructura de respuesta inesperada Cita.".into());
        }
    } else {
        return Err(format!("Error: {}", respuesta.status()).into());
    }
}

pub async fn hacer_mirror(
    clave_privada: &str,
    mirror_on: &str,
    token_autorizado: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    let cliente = inicializar_api();
    let billetera = inicializar_billetera(&clave_privada);

    let consulta = json!({
        "query": r#"
            mutation CreateOnchainMirrorTypedData($request: OnchainMirrorRequest!) {
                createOnchainMirrorTypedData(request: $request) {
      id
      expiresAt
      typedData {
        types {
          Mirror {
            name
            type
          }
        }
        domain {
          name
          chainId
          version
          verifyingContract
        }
        value {
          nonce
          metadataURI
          deadline
          profileId
          metadataURI
          pointedProfileId
          pointedPubId
          referrerProfileIds
          referrerPubIds
          referenceModuleData
        }
      }
    }
            }
        "#,
        "variables": {
            "request": {
                "mirrorOn": mirror_on,
            }
        }
    });

    println!("{}", mirror_on);

    let respuesta = cliente
        .post(API_LENS)
        .header("Authorization", format!("Bearer {}", token_autorizado))
        .header("Content-Type", "application/json")
        .json(&consulta)
        .send()
        .await?;

    if respuesta.status().is_success() {
        let json: serde_json::Value = respuesta.json().await?;
        println!("{}", json);
        if let Some(datos) = json["data"]["createOnchainMirrorTypedData"].as_object() {
            let datos_escritos = datos.get("typedData").and_then(|v| v.as_object()).unwrap();

            let domain = serde_json::from_value::<EIP712Domain>(
                datos_escritos.get("domain").cloned().unwrap(),
            )?;
            let types = serde_json::from_value::<BTreeMap<String, Vec<Eip712DomainType>>>(
                datos_escritos.get("types").cloned().unwrap(),
            )?;
            let value = serde_json::from_value::<BTreeMap<String, serde_json::Value>>(
                datos_escritos.get("value").cloned().unwrap(),
            )?;

            let firma = billetera
                .sign_typed_data(&TypedData {
                    domain,
                    types,
                    primary_type: "Mirror".to_string(),
                    message: value,
                })
                .await?;

            return Ok(propogar(
                datos.get("id").and_then(|v| v.as_str()).unwrap_or_default(),
                &firma.to_string(),
                token_autorizado,
            )
            .await?);
        } else {
            return Err("Estructura de respuesta inesperada Mirror.".into());
        }
    } else {
        return Err(format!("Error: {}", respuesta.status()).into());
    }
}

pub async fn hacer_publicacion(
    clave_privada: &str,
    contenido: String,
    token_autorizado: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    let cliente = inicializar_api();
    let billetera = inicializar_billetera(&clave_privada);

    let consulta = json!({
        "query": r#"
            mutation CreateOnchainPostTypedData($request: OnchainPostRequest!) {
                createOnchainPostTypedData(request: $request) {
      id
      expiresAt
      typedData {
        types {
          Post {
            name
            type
          }
        }
        domain {
          name
          chainId
          version
          verifyingContract
        }
        value {
          nonce
          deadline
          profileId
          contentURI
          actionModules
          actionModulesInitDatas
          referenceModule
          referenceModuleInitData
        }
      }
    }
            }
        "#,
        "variables": {
            "request": {
                "contentURI": contenido,
                "openActionModules": [{
                    "collectOpenAction": {
                        "simpleCollectOpenAction": {
                            "followerOnly": false
                        }
                    }
                }]
            }
        }
    });

    let respuesta = cliente
        .post(API_LENS)
        .header("Authorization", format!("Bearer {}", token_autorizado))
        .header("Content-Type", "application/json")
        .json(&consulta)
        .send()
        .await?;

    if respuesta.status().is_success() {
        let json: serde_json::Value = respuesta.json().await?;

        if let Some(datos) = json["data"]["createOnchainPostTypedData"].as_object() {
            let datos_escritos = datos.get("typedData").and_then(|v| v.as_object()).unwrap();

            let domain = serde_json::from_value::<EIP712Domain>(
                datos_escritos.get("domain").cloned().unwrap(),
            )?;
            let types = serde_json::from_value::<BTreeMap<String, Vec<Eip712DomainType>>>(
                datos_escritos.get("types").cloned().unwrap(),
            )?;
            let value = serde_json::from_value::<BTreeMap<String, serde_json::Value>>(
                datos_escritos.get("value").cloned().unwrap(),
            )?;

            let firma = billetera
                .sign_typed_data(&TypedData {
                    domain,
                    types,
                    primary_type: "Post".to_string(),
                    message: value,
                })
                .await?;

            return Ok(propogar(
                datos.get("id").and_then(|v| v.as_str()).unwrap_or_default(),
                &firma.to_string(),
                token_autorizado,
            )
            .await?);
        } else {
            return Err("Estructura de respuesta inesperada Publicacion.".into());
        }
    } else {
        return Err(format!("Error: {}", respuesta.status()).into());
    }
}

async fn propogar(
    id: &str,
    firma: &str,
    token_autorizado: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    let cliente = inicializar_api();
    let consulta = json!({
        "query": r#"
            mutation BroadcastOnchain($request: BroadcastRequest!) {
                broadcastOnchain(request: $request) {
                    ... on RelaySuccess {
        txId
      }
      ... on RelayError {
        reason
      }
                }
            }
        "#,
        "variables": {
            "request": {
                    "id": id,
                    "signature": firma
            }
        }
    });

    let respuesta = cliente
        .post(API_LENS)
        .header("Authorization", format!("Bearer {}", token_autorizado))
        .header("Content-Type", "application/json")
        .json(&consulta)
        .send()
        .await?;

    if respuesta.status().is_success() {
        let json: serde_json::Value = respuesta.json().await?;
        if let Some(_) = json["data"]["broadcastOnchain"]["txId"].as_str() {
            return Ok("RelaySuccess".to_string());
        } else {
            return Ok("RelayError".to_string());
        }
    } else {
        return Err(format!("Error: {}", respuesta.status()).into());
    }
}

pub async fn me_gusta(
    gusta_on: &str,
    token_autorizado: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    let cliente = inicializar_api();

    let consulta = json!({
        "query": r#"
            mutation AddReaction($request: ReactionRequest!) {
                addReaction(request: $request)
            }
        "#,
        "variables": {
            "request": {
                "for": gusta_on,
                "reaction": "UPVOTE"
              }
        }
    });

    let respuesta = cliente
        .post(API_LENS)
        .header("Authorization", format!("Bearer {}", token_autorizado))
        .header("Content-Type", "application/json")
        .json(&consulta)
        .send()
        .await?;

    if respuesta.status().is_success() {
        return Ok(gusta_on.to_string());
    } else {
        return Err(format!("Error: {}", respuesta.status()).into());
    }
}
