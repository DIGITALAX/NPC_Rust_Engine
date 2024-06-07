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

use crate::{API_LENS, AUTOGRAPH_DATA, LENS_HUB_PROXY, NPC_PUBLICATION};

static INIT: Once = Once::new();
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
        INIT.call_once(|| {
            dotenv().ok();
            let proveedor_url = format!(
                "https://polygon-mainnet.g.alchemy.com/v2/{}",
                var("ALCHEMY_API_KEY").expect("ALCHEMY_API_KEY not found")
            );
            let proveedor = Provider::<Http>::try_from(&proveedor_url).unwrap();
            PROVEEDOR = Some(Arc::new(proveedor));
        });
        PROVEEDOR.clone().expect("Proveedor no es inicializado")
    }
}

fn inicializar_api() -> Arc<Client> {
    unsafe {
        INIT.call_once(|| {
            CLIENTE_LENS = Some(Arc::new(Client::new()));
        });
        CLIENTE_LENS.clone().expect("Cliente no es inicializado")
    }
}

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
        INIT.call_once(|| {
            dotenv().ok();
            let direccion = LENS_HUB_PROXY.parse::<Address>().unwrap();
            let abi: Abi = from_str(include_str!("./../../abis/LensHubProxy.json")).unwrap();
            let billetera = var(clave_privada)
                .expect("PRIVATE_KEY not found")
                .parse::<LocalWallet>()
                .unwrap();
            let cliente = SignerMiddleware::new(proveedor.clone(), billetera);
            let contrato = Contract::new(direccion, abi, Arc::new(cliente.clone()));
            LENS_HUB_PROXY_CONTRATO = Some(Arc::new(contrato));

            let direccion = AUTOGRAPH_DATA.parse::<Address>().unwrap();
            let abi: Abi = from_str(include_str!("./../../abis/AutographData.json")).unwrap();
            let contrato = Contract::new(direccion, abi, Arc::new(cliente.clone()));
            AUTOGRAPH_DATA_CONTRATO = Some(Arc::new(contrato));

            let direccion = NPC_PUBLICATION.parse::<Address>().unwrap();
            let abi: Abi = from_str(include_str!("./../../abis/NPCPublication.json")).unwrap();
            let contrato = Contract::new(direccion, abi, Arc::new(cliente));
            NPC_PUBLICATION_CONTRATO = Some(Arc::new(contrato));
        });
        (
            LENS_HUB_PROXY_CONTRATO
                .clone()
                .expect("Contrato no es inicializado"),
            AUTOGRAPH_DATA_CONTRATO
                .clone()
                .expect("Contrato no es inicializado"),
            NPC_PUBLICATION_CONTRATO
                .clone()
                .expect("Contrato no es inicializado"),
        )
    }
}
