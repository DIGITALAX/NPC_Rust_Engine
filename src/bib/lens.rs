use dotenv::dotenv;
use ethers::{
    abi::{Abi, Address},
    contract::Contract,
    middleware::SignerMiddleware,
    providers::{Http, Provider},
    signers::LocalWallet,
};
use reqwest::Client;
use serde_json::{from_str, json};
use std::{
    env::var,
    error::Error,
    sync::{Arc, Once},
};

use crate::{GenerarDesafioConsulta, API_LENS, AUTOGRAPH_DATA, LENS_HUB_PROXY, NPC_PUBLICATION};

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
            let proveedor =
                Provider::<Http>::try_from(&proveedor_url).expect("Error al crear proveedor");
            PROVEEDOR = Some(Arc::new(proveedor));
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

// pub async fn conectarse_lens(perfil_id: &str, direccion: &str) {
//     let cliente = inicializar_api();

//     let request = GenerarDesafioConsulta {
//         para: perfil_id.to_string(),
//         signedBy: direccion.to_string(),
//     };
//     let response = cliente
//         .post(API_LENS)
//         .json(&request)
//         .send()
//         .await?
//         .json::<ChallengeResponse>()
//         .await?;

//     let request = AuthenticateRequest {
//         id: challenge_id.to_string(),
//         signature: signature.to_string(),
//     };
//     let response = cliente
//         .post(API_LENS)
//         .json(&request)
//         .send()
//         .await?
//         .json::<AuthResponse>()
//         .await?;

//     Ok(())
// }

pub async fn hacer_consulta(perfil_id: &str) -> Result<u64, Box<dyn Error>> {
    let cliente = inicializar_api();

    let consulta = format!(
        r#"
        query {{
          feed(request: {{
            where: {{
              for: "{}"
            }}
          }}) {{
            items {{
              id
            }}
            pageInfo {{
              next
            }}
          }}
        }}
    "#,
        perfil_id
    );

    let cuerpo = json!({
        "query": consulta,
    });

    let respuesta = cliente.post(API_LENS).json(&cuerpo).send().await?;

    if respuesta.status().is_success() {
        let json: serde_json::Value = respuesta.json().await?;
        if let Some(items) = json["data"]["feed"]["items"].as_array() {
            if !items.is_empty() {
                if let Some(id) = items[0]["id"].as_str() {
                    let id_num: u64 = id.parse()?;
                    return Ok(id_num);
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
                    Ok(wallet) => wallet,
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
