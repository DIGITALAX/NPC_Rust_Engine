use crate::bib::types::Llama;
use ollama_rs::{generation::completion::request::GenerationRequest, Ollama};
use std::{error::Error, process::Command};

impl Llama {
    pub async fn llamar_llama(&self, prompt: &str) -> Result<String, Box<dyn Error + Send + Sync>> {
        let ollama = Ollama::new("http://localhost".to_string(), 11434);
        let model = "llama3:70b".to_string();

        let list_models_output = Command::new("./ollama").arg("list").output()?;
        println!("lista de los modelos {:?}", list_models_output);

        let res = ollama
            .generate(GenerationRequest::new(model, prompt.to_string()))
            .await;

        match res {
            Ok(response) => Ok(response.response),
            Err(e) => Err(Box::new(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("Error con Ollama {}", e),
            ))),
        }
    }
}
