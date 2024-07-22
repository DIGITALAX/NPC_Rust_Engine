use crate::bib::types::Llama;
use std::{error::Error, process::Command};
use tokio::task;
use shell_escape::escape;

impl Llama {
    pub async fn llamar_llama(&self, prompt: &str) -> Result<String, Box<dyn Error + Send + Sync>> {
        let prompt = escape(prompt.into()).to_string();
        
        let output = task::spawn_blocking(move || {
            Command::new("bash")
                .arg("-c")
                .arg(format!("python3 ~/project/src/llama3_runner.py {}", prompt))
                .output()
        })
        .await??;

        if output.status.success() {
            let response_json = String::from_utf8(output.stdout)?;
            let response: serde_json::Value = serde_json::from_str(&response_json)?;
            Ok(response["response"].as_str().unwrap_or("").to_string())
        } else {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            Err(Box::new(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("Comando falló. stdout: {}, stderr: {}", stdout, stderr),
            )))
        }
    }
}
