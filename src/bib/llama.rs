use crate::bib::types::Llama;
use log::info;
use reqwest::Client;
use serde_json::Value;
use std::{env, error::Error};

impl Llama {
    pub async fn llamar_llama(&self, prompt: &str) -> Result<String, Box<dyn Error + Send + Sync>> {
        let ollama_clave = env::var("OLLAMA_KEY").expect("OLLAMA_KEY no está configurada en .env");
        let url = "https://glorious-eft-deeply.ngrok-free.app/run_llama";
        let cliente = Client::new();
        let payload = serde_json::json!({
            "api_key": ollama_clave,
            "prompt": prompt.trim()
        });


        let res = cliente.post(url).json(&payload).send().await?;

        if res.status().is_success() {
            let response_body: Value = res.json().await?;
            if let Some(response) = response_body.get("response").and_then(|v| v.as_str()) {
                info!("{}", response);
                Ok(response.to_string())
            } else {
                Err(Box::new(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "Respuesta inesperada: falta 'response' o no es una cadena",
                )))
            }
        } else {
            Err(Box::new(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("Error en la solicitud: {}", res.status()),
            )))
        }
    }
}
