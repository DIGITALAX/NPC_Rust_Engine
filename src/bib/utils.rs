use rand::Rng;
use std::error::Error;

use crate::bib::{ipfs, types::IpfsRespuesta};

pub fn between(min: f32, max: f32) -> f32 {
    let mut rng = rand::thread_rng();
    rng.gen_range(min..=max)
}

pub async fn subir_ipfs(datos: String) -> Result<IpfsRespuesta, Box<dyn Error>> {
    let cliente = ipfs::cliente();
    let aut_encoded = ipfs::autenticacion();

    let forma = reqwest::multipart::Form::new().part(
        "file",
        reqwest::multipart::Part::text(datos.clone()).file_name("data.json"),
    );

    let respuesta = cliente
        .post("https://ipfs.infura.io:5001/api/v0/add")
        .header("Authorization", format!("Basic {}", aut_encoded))
        .multipart(forma)
        .send()
        .await?;

    let texto_respuesta = respuesta.text().await?;
    let ipfs_response: IpfsRespuesta = serde_json::from_str(&texto_respuesta)?;

    Ok(ipfs_response)
}

