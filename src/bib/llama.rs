use crate::bib::types::Llama;
use std::{fs,error::Error, process::Command, path::Path};
use tokio::task;
use shell_escape::escape;

fn install_ollama() -> Result<(), Box<dyn std::error::Error>> {
    let ollama_path = Path::new("/opt/render/project/src/ollama");

    if ollama_path.exists() {
        println!("Ollama is already installed at {:?}", ollama_path);
        return Ok(());
    }

    if let Some(parent) = ollama_path.parent() {
        fs::create_dir_all(parent)?;
    }

    let output = Command::new("curl")
        .arg("-L")
        .arg("https://ollama.com/download/ollama-linux-amd64")
        .arg("-o")
        .arg(ollama_path.to_str().unwrap())
        .output()?;

    if !output.status.success() {
        return Err(format!(
            "Failed to download ollama: {}",
            String::from_utf8_lossy(&output.stderr)
        ).into());
    }

    Command::new("chmod")
        .arg("+x")
        .arg(ollama_path.to_str().unwrap())
        .output()?;

    println!("Ollama installed successfully at {:?}", ollama_path);

    Ok(())
}



impl Llama {
    pub async fn llamar_llama(&self, prompt: &str) -> Result<String, Box<dyn Error + Send + Sync>> {
        install_ollama().expect("Failed to install Ollama");

        let prompt = escape(prompt.into()).to_string();
        
        let output = task::spawn_blocking(move || {
            Command::new("bash")
                .arg("-c")
                .arg(format!("python3 llama3_runner.py {}", prompt))
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
