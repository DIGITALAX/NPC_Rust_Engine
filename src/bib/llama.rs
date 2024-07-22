use reqwest::Client;

use crate::{bib::types::Llama, PromptRequisito, PromptRespuesta};
use std::{env, error::Error};

impl Llama {
    pub async fn llamar_llama(&self, prompt: &str) -> Result<String, Box<dyn Error + Send + Sync>> {
        let ollama_clave = env::var("OLLAMA_KEY").expect("OLLAMA_KEY no está configurada en .env");
        let url = "https://ollama-26q5.onrender.com/generate";
        let cliente = Client::new();
        let prompt = PromptRequisito {
            text: prompt.to_string(),
        };

        let res = cliente
            .post(url)
            .header("Authorization", ollama_clave)
            .json(&prompt)
            .send()
            .await?;

        if res.status().is_success() {
            let response_body: PromptRespuesta = res.json().await?;
            Ok(response_body.response)
        } else {
            Err(Box::new(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("Error en la solicitud: {}", res.status()),
            )))
        }
    }
}
