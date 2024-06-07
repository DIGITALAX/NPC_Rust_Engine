use crate::bib::types::{Llama, PromptRespuesta};
use std::{error::Error, process::Command};

impl Llama {
    pub async fn llamar_llama(&self, prompt: &str) -> Result<String, Box<dyn Error>> {
        let output = Command::new("python3")
            .arg("llama3_runner.py")
            .arg(prompt)
            .output()?;

        if output.status.success() {
            let response_json = String::from_utf8(output.stdout)?;
            let response: PromptRespuesta = serde_json::from_str(&response_json)?;
            Ok(response.respuesta)
        } else {
            let error_message = String::from_utf8(output.stderr)?;
            Err(Box::new(std::io::Error::new(
                std::io::ErrorKind::Other,
                error_message,
            )))
        }
    }
}
