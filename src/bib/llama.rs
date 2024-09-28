use crate::{
    bib::types::{Llama, LlamaOpciones},
    LensType,
};
use ethers::{abi::Tokenizable, types::U256};
use reqwest::Client;
use std::{env, error::Error, time::Duration};

impl Llama {
    pub async fn llamar_llama(
        &self,
        nombre: &str,
        escena: &str,
        metadata_uri: String,
        locale: &String,
        eleccion: LensType,
        comentario_perfil: U256,
        comentario_pub: U256,
        perfil_id: U256,
        coleccion_id: U256,
        pagina: u8,
        prompt: &str,
        imagenes: Option<String>,
        opciones: LlamaOpciones,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        let ollama_clave = env::var("OLLAMA_KEY").expect("OLLAMA_KEY no está configurada en .env");

        let url_iniciar = "https://glorious-eft-deeply.ngrok-free.app/run_llama";

        let cliente = Client::builder()
            .danger_accept_invalid_certs(true)
            .danger_accept_invalid_hostnames(true)
            .timeout(Duration::from_secs(10000))
            .connect_timeout(Duration::from_secs(10000))
            .read_timeout(Duration::from_secs(10000))
            .pool_idle_timeout(Duration::from_secs(10000))
            .pool_max_idle_per_host(10000)
            .use_rustls_tls()
            .no_gzip()
            .no_brotli()
            .no_deflate()
            .no_proxy()
            .build()?;

        let payload_inicial = serde_json::json!({
            "api_key": ollama_clave,
            "prompt": prompt.trim(),
            "model": "E:\\dev\\llama.cpp\\models\\8B\\Meta-Llama-3.1-8B-Instruct-Q8_0.gguf",
            "images": imagenes,
            "options": opciones,
            "id": nombre,
            "escena": escena,
            "metadata_uri": metadata_uri,
            "locale": locale,
            "eleccion": eleccion.into_token(),
            "comentario_perfil": comentario_perfil,
            "comentario_pub": comentario_pub,
            "perfil_id":perfil_id,
            "coleccion_id": coleccion_id,
            "pagina": pagina,
        });

        let res_inicial = cliente
            .post(url_iniciar)
            .header("Content-Type", "application/json; charset=UTF-8")
            .json(&payload_inicial)
            .send()
            .await?;

        if res_inicial.status() == 200 {
            println!("Solicitud exitosa, servidor respondió con 200.");
            return Ok(());
        } else {
            return Err(Box::new(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!(
                    "Error en la solicitud: código de estado {:?}",
                    res_inicial.status()
                ),
            )));
        }
    }
}
