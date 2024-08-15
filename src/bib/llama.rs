use crate::bib::types::{Llama, LlamaOpciones, LlamaRespuesta};
use reqwest::Client;
use serde_json::Value;
use std::{env, error::Error};

impl Llama {
    pub async fn llamar_llama(
        &self,
        prompt: &str,
        imagenes: Option<String>,
        opciones: LlamaOpciones,
    ) -> Result<LlamaRespuesta, Box<dyn Error + Send + Sync>> {
        let ollama_clave = env::var("OLLAMA_KEY").expect("OLLAMA_KEY no está configurada en .env");
        let url = "https://glorious-eft-deeply.ngrok-free.app/run_llama";
        // let url = "http://0.0.0.0:5000/run_llama";
        let cliente = Client::new();

        let payload = serde_json::json!({
            "api_key": ollama_clave,
            "prompt": prompt.trim(),
            "model": "llama3:8b",
            "images": imagenes,
            "options": opciones
        });

        let res = cliente.post(url).json(&payload).send().await?;
        if res.status().is_success() {
            let response_body: Value = res.json().await?;
            if let Some(response) = response_body.get("response") {
                let res: String = response
                    .get("response")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string();

                let filtered_response = res
                    .lines()
                    .filter(|line| {
                        !line.contains(
                            "failed to get console mode for stdout: The handle is invalid.",
                        ) && !line.contains(
                            "failed to get console mode for stderr: The handle is invalid.",
                        )
                    })
                    .collect::<Vec<&str>>()
                    .join("\n");

                Ok(LlamaRespuesta {
                    prompt: filtered_response,
                    json: response.clone(),
                })
            } else {
                Err(Box::new(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "Respuesta inesperada: falta 'response'.",
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
