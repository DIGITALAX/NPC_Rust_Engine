use crate::bib::constants::{AUTOGRAPH_CATALOG, LENS_CHAIN_ID, LENS_RPC_URL, SPECTATOR_REWARDS};
use dotenv::{dotenv, from_filename, var};
use ethers::{
    abi::{Abi, Address},
    addressbook::Chain,
    contract::{Contract, ContractInstance},
    core::k256::ecdsa::SigningKey,
    middleware::SignerMiddleware,
    providers::{Http, Provider, ProviderExt},
    signers::{LocalWallet, Signer, Wallet},
};
use reqwest::Client;
use serde_json::from_str;
use std::sync::{Arc, Mutex, Once};

static INIT_PROVIDER: Once = Once::new();
static INIT_LENS: Once = Once::new();
static AUTOGRAPH_CATALOG_CONTRATO: Mutex<
    Option<Arc<Contract<SignerMiddleware<Arc<Provider<Http>>, LocalWallet>>>>,
> = Mutex::new(None);
static SPECTATOR_REWARDS_CONTRATO: Mutex<
    Option<Arc<Contract<SignerMiddleware<Arc<Provider<Http>>, LocalWallet>>>>,
> = Mutex::new(None);
static PROVIDER: Mutex<Option<Arc<Provider<Http>>>> = Mutex::new(None);
static LENS_CLIENT: Mutex<Option<Arc<Client>>> = Mutex::new(None);
static WALLET: Mutex<Option<LocalWallet>> = Mutex::new(None);

pub fn initialize_provider() -> Arc<Provider<Http>> {
    INIT_PROVIDER.call_once(|| {
        dotenv().ok();
        let chain_id = *LENS_CHAIN_ID;
        let chain: Chain = unsafe { std::mem::transmute(chain_id as u64) };
        let mut provider =
            Provider::<Http>::try_from(LENS_RPC_URL).expect("Error in creating the provider");
        let provider = provider.set_chain(chain).clone();
        *PROVIDER.lock().unwrap() = Some(Arc::new(provider));
    });

    PROVIDER
        .lock()
        .unwrap()
        .clone()
        .expect("Provider not initialized")
}

pub fn initialize_api() -> Arc<Client> {
    INIT_LENS.call_once(|| {
        *LENS_CLIENT.lock().unwrap() = Some(Arc::new(Client::new()));
    });

    LENS_CLIENT
        .lock()
        .unwrap()
        .clone()
        .expect("Client not initialized")
}

pub fn initialize_contracts(
    private_key: &str,
) -> Option<(
    Arc<
        ContractInstance<
            Arc<SignerMiddleware<Arc<Provider<Http>>, Wallet<SigningKey>>>,
            SignerMiddleware<Arc<Provider<Http>>, Wallet<SigningKey>>,
        >,
    >,
    Arc<
        ContractInstance<
            Arc<SignerMiddleware<Arc<Provider<Http>>, Wallet<SigningKey>>>,
            SignerMiddleware<Arc<Provider<Http>>, Wallet<SigningKey>>,
        >,
    >,
)> {
    dotenv().ok();
    let provider = initialize_provider();

    let wallet = match initialize_wallet(private_key) {
        Some(wallet) => wallet,
        None => {
            eprintln!("Wallet initialization failed. Skipping agent creation.");
            return None;
        }
    };

    let client = Arc::new(SignerMiddleware::new(provider.clone(), wallet));

    let autograph_address = AUTOGRAPH_CATALOG
        .parse::<Address>()
        .expect("Error in parsing AUTOGRAPH_CATALOG_CONTRATO");
    let autograph_abi: Abi = from_str(include_str!("./../../abis/AutographCatalog.json"))
        .expect("Error in loading Autograph Data ABI");
    let autograph_contract = Contract::new(autograph_address, autograph_abi, client.clone());
    *AUTOGRAPH_CATALOG_CONTRATO.lock().unwrap() = Some(Arc::new(autograph_contract));

    let spectator_address = SPECTATOR_REWARDS
        .parse::<Address>()
        .expect("Error in parsing SPECTATOR_REWARDS_CONTRATO");
    let spectator_abi: Abi = from_str(include_str!("./../../abis/SpectatorRewards.json"))
        .expect("Error in loading Autograph Data ABI");
    let spectator_contract = Contract::new(spectator_address, spectator_abi, client.clone());
    *SPECTATOR_REWARDS_CONTRATO.lock().unwrap() = Some(Arc::new(spectator_contract));

    Some((
        AUTOGRAPH_CATALOG_CONTRATO
            .lock()
            .unwrap()
            .clone()
            .expect("AUTOGRAPH_CATALOG_CONTRATO not initialized"),
        SPECTATOR_REWARDS_CONTRATO
            .lock()
            .unwrap()
            .clone()
            .expect("SPECTATOR_REWARDS_CONTRATO not initialized"),
    ))
}

pub fn initialize_wallet(private_key: &str) -> Option<LocalWallet> {
    from_filename(".env").ok();
    match var(format!("{}", private_key.to_string())) {
        Ok(key) => match key.parse::<LocalWallet>() {
            Ok(mut wallet) => {
                let chain_id = *LENS_CHAIN_ID;
                wallet = wallet.with_chain_id(chain_id);
                *WALLET.lock().unwrap() = Some(wallet.clone());
                Some(wallet)
            }
            Err(e) => {
                eprintln!("Error in parsing private key: {:?}", e);
                None
            }
        },
        Err(_) => {
            eprintln!("PRIVATE_KEY not found in .env for agent_{}", private_key);
            None
        }
    }
}
