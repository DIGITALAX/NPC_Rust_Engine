use base64::{engine::general_purpose::STANDARD, Engine as _};
use dotenv::dotenv;
use reqwest::Client;
use std::{
    env,
    sync::{Arc, Once},
};

static INIT: Once = Once::new();
static mut CLIENTE: Option<Arc<Client>> = None;

pub fn cliente() -> Arc<Client> {
    unsafe {
        INIT.call_once(|| {
            dotenv().ok();
            let client = Client::new();
            CLIENTE = Some(Arc::new(client));
        });
        CLIENTE.clone().expect("Cliente no es inicializado")
    }
}

pub fn autenticacion() -> String {
    dotenv().ok();
    let id = env::var("INFURA_PROJECT_ID").expect("INFURA_PROJECT_ID not set");
    let clave = env::var("INFURA_PROJECT_SECRET").expect("INFURA_PROJECT_SECRET no es configurado");
    let aut = format!("{}:{}", id, clave);
    STANDARD.encode(aut)
}
