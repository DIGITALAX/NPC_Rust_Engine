use base64::{engine::general_purpose::STANDARD, Engine as _};
use dotenv::dotenv;
use reqwest::Client;
use std::{
    env,
    sync::{Arc, OnceLock},
};

static CLIENTE: OnceLock<Arc<Client>> = OnceLock::new();

pub fn cliente() -> Arc<Client> {
    CLIENTE
        .get_or_init(|| {
            dotenv().ok();
            Arc::new(Client::new())
        })
        .clone()
}

pub fn autenticacion() -> String {
    dotenv().ok();
    let id = env::var("INFURA_PROJECT_ID").expect("INFURA_PROJECT_ID not set");
    let clave = env::var("INFURA_PROJECT_SECRET").expect("INFURA_PROJECT_SECRET no es configurado");
    let aut = format!("{}:{}", id, clave);
    STANDARD.encode(aut)
}
