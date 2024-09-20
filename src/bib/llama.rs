use crate::bib::types::{Llama, LlamaOpciones, LlamaRespuesta};
use reqwest::Client;
use serde_json::Value;
use std::{env, error::Error};

use super::utils::quitar_diacriticos;

impl Llama {
    pub async fn llamar_llama(
        &self,
        prompt: &str,
        imagenes: Option<String>,
        opciones: LlamaOpciones,
    ) -> Result<LlamaRespuesta, Box<dyn Error + Send + Sync>> {
        let ollama_clave = env::var("OLLAMA_KEY").expect("OLLAMA_KEY no está configurada en .env");
        let url = "https://glorious-eft-deeply.ngrok-free.app/run_llama";
        // let url = "http://127.0.0.1:5000/run_llama";
        let cliente = Client::builder()
            .danger_accept_invalid_certs(true)
            .build()?;

        let payload = serde_json::json!({
            "api_key": ollama_clave,
            "prompt": prompt.trim(),
            "model": "G:\\dev\\fresh\\ollama\\llm\\llama.cpp\\models\\8B\\Meta-Llama-3.1-8B-Instruct-Q8_0.gguf",
            "images": imagenes,
            "options": opciones
        });

        let res = cliente.post(url).json(&payload).send().await?;
        if res.status().is_success() {
            let response_body: Value = res.json().await?;

            if let Some(response) = response_body.get("response") {
                let res: String = quitar_diacriticos(
                    response
                        .get("output")
                        .and_then(|v: &Value| v.as_str())
                        .unwrap_or_default(),
                );

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
                    response: quitar_diacriticos(&filtered_response),
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
                format!("Error en la solicitud: {:?}", res.status()),
            )))
        }
    }
}
