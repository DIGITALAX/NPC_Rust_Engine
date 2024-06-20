use dotenv::dotenv;
use ethers::signers::Signer;
use ethers::{
    abi::{Abi, Address},
    addressbook::Chain,
    contract::Contract,
    middleware::SignerMiddleware,
    providers::{Http, Provider, ProviderExt},
    signers::LocalWallet,
    types::U256,
};
use rand::Rng;
use reqwest::Client;
use serde_json::{from_str, json};
use std::{
    env::var,
    error::Error,
    sync::{Arc, Once},
};

use crate::{API_LENS, AUTOGRAPH_DATA, LENS_HUB_PROXY, NPC_PUBLICATION};

static INIT_PROVEEDOR: Once = Once::new();
static INIT_CONTRATOS: Once = Once::new();
static INIT_LENS: Once = Once::new();
static mut LENS_HUB_PROXY_CONTRATO: Option<
    Arc<Contract<SignerMiddleware<Arc<Provider<Http>>, LocalWallet>>>,
> = None;
static mut AUTOGRAPH_DATA_CONTRATO: Option<
    Arc<Contract<SignerMiddleware<Arc<Provider<Http>>, LocalWallet>>>,
> = None;
static mut NPC_PUBLICATION_CONTRATO: Option<
    Arc<Contract<SignerMiddleware<Arc<Provider<Http>>, LocalWallet>>>,
> = None;
static mut PROVEEDOR: Option<Arc<Provider<Http>>> = None;
static mut CLIENTE_LENS: Option<Arc<Client>> = None;

pub fn inicializar_proveedor() -> Arc<Provider<Http>> {
    unsafe {
        INIT_PROVEEDOR.call_once(|| {
            dotenv().ok();
            let proveedor_url = format!(
                "https://polygon-amoy.g.alchemy.com/v2/{}",
                var("ALCHEMY_AMOY_KEY").expect("ALCHEMY_API_KEY not found")
            );
            let mut proveedor =
                Provider::<Http>::try_from(&proveedor_url).expect("Error al crear proveedor");
            PROVEEDOR = Some(Arc::new(proveedor.set_chain(Chain::PolygonAmoy).clone()));
        });
        PROVEEDOR.clone().expect("Proveedor no es inicializado")
    }
}

fn inicializar_api() -> Arc<Client> {
    unsafe {
        INIT_LENS.call_once(|| {
            CLIENTE_LENS = Some(Arc::new(Client::new()));
        });
        CLIENTE_LENS.clone().expect("Cliente no es inicializado")
    }
}

pub async fn coger_comentario(perfil_id: &str) -> Result<(String, U256, U256), Box<dyn Error>> {
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
                            }
                        }

                    }
                }
            }
        "#,
        "variables": {
            "request": {
                "where": {
                    "from": [perfil_id.to_string()]
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
                let indice_aleatorio = rng.gen_range(0..items.len());

                if let Some(id) = items[indice_aleatorio]["id"].as_str() {
                    if let Some(hex_str) = id.split('-').nth(0) {
                        let comentario_perfil = U256::from_str_radix(&hex_str[2..], 16)?;

                        if let Some(hex_str) = id.split('-').nth(1) {
                            let comentario_pub = U256::from_str_radix(&hex_str[2..], 16)?;

                            if let Some(contenido) = items[indice_aleatorio]["content"].as_str() {
                                return Ok((
                                    contenido.to_string(),
                                    comentario_perfil,
                                    comentario_pub,
                                ));
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
                return Err("No se encontraron elementos.".into());
            }
        } else {
            return Err("Estructura de respuesta inesperada.".into());
        }
    } else {
        return Err(format!("Error: {}", respuesta.status()).into());
    }
}

pub async fn hacer_consulta(perfil_id: &str) -> Result<U256, Box<dyn Error>> {
    let cliente = inicializar_api();
    let consulta = json!({
        "query": r#"
            query Publications($request: PublicationsRequest!) {
                publications(request: $request) {
                    items {
                        ... on Post {
                            id
                        }

                    }
                }
            }
        "#,
        "variables": {
            "request": {
                "where": {
                    "from": [perfil_id.to_string()]
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
                if let Some(id) = items[0]["id"].as_str() {
                    if let Some(hex_str) = id.split('-').nth(1) {
                        let numero = u32::from_str_radix(&hex_str[2..], 16)?;
                        return Ok(numero.into());
                    } else {
                        return Err("Error con el Hex".into());
                    }
                } else {
                    return Err("ID no encontrado o no es una cadena de texto.".into());
                }
            } else {
                return Err("No se encontraron elementos.".into());
            }
        } else {
            return Err("Estructura de respuesta inesperada.".into());
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
    Arc<Contract<SignerMiddleware<Arc<Provider<Http>>, LocalWallet>>>,
) {
    unsafe {
        let proveedor = inicializar_proveedor();
        INIT_CONTRATOS.call_once(|| {
            dotenv().ok();

            let direccion = match LENS_HUB_PROXY.parse::<Address>() {
                Ok(addr) => addr,
                Err(e) => {
                    panic!("Error al parsear LENS_HUB_PROXY: {:?}", e);
                }
            };

            let abi: Abi = match from_str(include_str!("./../../abis/LensHubProxy.json")) {
                Ok(a) => a,
                Err(e) => {
                    panic!("Error al cargar LensHubProxy ABI: {:?}", e);
                }
            };

            let billetera = match var(clave_privada.replace("-", "_")) {
                Ok(key) => match key.parse::<LocalWallet>() {
                    Ok(mut wallet) => {
                        wallet = wallet.with_chain_id(Chain::PolygonAmoy);
                        wallet
                    }
                    Err(e) => panic!("Error al parsear la clave privada: {:?}", e),
                },
                Err(e) => panic!("PRIVATE_KEY not found: {:?}", e),
            };

            let cliente = SignerMiddleware::new(proveedor.clone(), billetera);

            let contrato = Contract::new(direccion, abi, Arc::new(cliente.clone()));
            LENS_HUB_PROXY_CONTRATO = Some(Arc::new(contrato));

            let direccion = match AUTOGRAPH_DATA.parse::<Address>() {
                Ok(addr) => addr,
                Err(e) => {
                    panic!("Error al parsear AUTOGRAPH_DATA: {:?}", e);
                }
            };

            let abi: Abi = match from_str(include_str!("./../../abis/AutographData.json")) {
                Ok(a) => a,
                Err(e) => {
                    panic!("Error al cargar AutographData ABI: {:?}", e);
                }
            };

            let contrato = Contract::new(direccion, abi, Arc::new(cliente.clone()));
            AUTOGRAPH_DATA_CONTRATO = Some(Arc::new(contrato));

            let direccion = match NPC_PUBLICATION.parse::<Address>() {
                Ok(addr) => addr,
                Err(e) => {
                    panic!("Error al parsear NPC_PUBLICATION: {:?}", e);
                }
            };

            let abi: Abi = match from_str(include_str!("./../../abis/NPCPublication.json")) {
                Ok(a) => a,
                Err(e) => {
                    panic!("Error al cargar NPCPublication ABI: {:?}", e);
                }
            };

            let contrato = Contract::new(direccion, abi, Arc::new(cliente));

            NPC_PUBLICATION_CONTRATO = Some(Arc::new(contrato));
        });

        let lens_hub_contrato = LENS_HUB_PROXY_CONTRATO
            .clone()
            .expect("LENS_HUB_PROXY_CONTRATO not initialized");
        let autograph_data_contrato = AUTOGRAPH_DATA_CONTRATO
            .clone()
            .expect("AUTOGRAPH_DATA_CONTRATO not initialized");
        let npc_publication_contrato = NPC_PUBLICATION_CONTRATO
            .clone()
            .expect("NPC_PUBLICATION_CONTRATO not initialized");

        (
            lens_hub_contrato,
            autograph_data_contrato,
            npc_publication_contrato,
        )
    }
}
