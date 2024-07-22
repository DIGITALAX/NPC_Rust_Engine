use crate::bib::types::Llama;
use ollama_rs::{generation::completion::request::GenerationRequest, Ollama};
use std::error::Error;

impl Llama {
    pub async fn llamar_llama(&self, prompt: &str) -> Result<String, Box<dyn Error + Send + Sync>> {
        let prompt = prompt.to_string();
        let ollama = Ollama::default();
        let model = "llama3:latest".to_string();
        let request = GenerationRequest::new(model, prompt);

        match ollama.generate(request).await {
            Ok(response) => Ok(response.response),
            Err(e) => Err(Box::new(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("Error generando respuesta: {}", e),
            ))),
        }
    }
}
