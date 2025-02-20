use crate::bib::constants::AUTOGRAPH_DATA;

use dotenv::dotenv;
use ethers::{
    abi::{Abi, Address},
    addressbook::Chain,
    contract::Contract,
    middleware::SignerMiddleware,
    providers::{Http, Provider, ProviderExt},
    signers::{LocalWallet, Signer},
};
use reqwest::Client;
use serde_json::from_str;
use std::{
    env::var,
    sync::{Arc, OnceLock},
};

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

pub fn inicializar_contrato(
    clave_privada: &str,
) -> Arc<Contract<SignerMiddleware<Arc<Provider<Http>>, LocalWallet>>> {
    dotenv().ok();
    let proveedor = inicializar_proveedor();
    let billetera = inicializar_billetera(clave_privada);
    let cliente = Arc::new(SignerMiddleware::new(proveedor.clone(), billetera.clone()));

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

    autograph_data_contrato
}
