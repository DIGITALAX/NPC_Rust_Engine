use dotenv::dotenv;
use ethers::{
    abi::{Abi, Address},
    contract::Contract,
    middleware::SignerMiddleware,
    providers::{Http, Provider},
    signers::LocalWallet,
};
use serde_json::from_str;
use std::{
    env::var,
    sync::{Arc, Once},
};

use crate::LENS_HUB_PROXY;

static INIT: Once = Once::new();
static mut CONTRATO: Option<Arc<Contract<SignerMiddleware<Arc<Provider<Http>>, LocalWallet>>>> =
    None;
static mut PROVEEDOR: Option<Arc<Provider<Http>>> = None;

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

pub fn inicializar_contrato(
    clave_privada: &str,
) -> Arc<Contract<SignerMiddleware<Arc<Provider<Http>>, LocalWallet>>> {
    unsafe {
        let proveedor = inicializar_proveedor();
        INIT.call_once(|| {
            dotenv().ok();
            let direccion = LENS_HUB_PROXY
                .parse::<Address>()
                .unwrap();
            let abi: Abi = from_str(include_str!("./../../abis/LensHubProxy.json")).unwrap();
            let billetera = var(clave_privada)
                .expect("PRIVATE_KEY not found")
                .parse::<LocalWallet>()
                .unwrap();
            let cliente = SignerMiddleware::new(proveedor.clone(), billetera);
            let contrato = Contract::new(direccion, abi, Arc::new(cliente));
            CONTRATO = Some(Arc::new(contrato));
        });
        CONTRATO.clone().expect("Contrato no es inicializado")
    }
}
