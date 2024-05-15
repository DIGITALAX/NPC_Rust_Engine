use base64::{engine::general_purpose::STANDARD, Engine as _};
use rand::Rng;
use reqwest::multipart::{Form, Part};
use serde_json::from_str;
use std::error::Error;

use crate::bib::{
    ipfs::{autenticacion, cliente},
    types::IpfsRespuesta,
};

pub fn between(min: f32, max: f32) -> f32 {
    let mut rng = rand::thread_rng();
    rng.gen_range(min..=max)
}

pub async fn subir_ipfs(datos: String) -> Result<IpfsRespuesta, Box<dyn Error>> {
    let cliente = cliente();
    let aut_encoded = autenticacion();

    let forma = Form::new().part("file", Part::text(datos.clone()).file_name("data.json"));

    let respuesta = cliente
        .post("https://ipfs.infura.io:5001/api/v0/add")
        .header("Authorization", format!("Basic {}", aut_encoded))
        .multipart(forma)
        .send()
        .await?;

    let texto_respuesta = respuesta.text().await?;
    let ipfs_respuesta: IpfsRespuesta = from_str(&texto_respuesta)?;

    Ok(ipfs_respuesta)
}

pub async fn subir_ipfs_imagen(base64_data: &str) -> Result<IpfsRespuesta, Box<dyn Error>> {
    let cliente = cliente();
    let aut_encoded = autenticacion();

    let imagen_bytes = STANDARD.decode(base64_data)?;

    let form = Form::new().part("file", Part::bytes(imagen_bytes).file_name("image.png"));

    let respuesta = cliente
        .post("https://ipfs.infura.io:5001/api/v0/add")
        .header("Authorization", format!("Basic {}", aut_encoded))
        .multipart(form)
        .send()
        .await?;

    let texto_respuesta = respuesta.text().await?;
    let ipfs_respuesta: IpfsRespuesta = from_str(&texto_respuesta)?;

    Ok(ipfs_respuesta)
}
