use crate::{
    bib::{
        ipfs::{autenticacion, cliente},
        types::IpfsRespuesta,
    },
    LensTokens, API_LENS,
};
use base64::{engine::general_purpose::STANDARD, Engine as _};
use ethers::{
    signers::{LocalWallet, Signer},
    utils::hex,
};
use rand::Rng;
use reqwest::{
    multipart::{Form, Part},
    Client,
};
use serde_json::{from_str, json};
use std::{error::Error, num::ParseIntError, sync::Arc};

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

pub async fn refrescar(
    cliente: Arc<Client>,
    token_refrescado: &str,
    token_autorizado: &str,
) -> Result<LensTokens, Box<dyn std::error::Error>> {
    let consulta = json!({
        "query": r#"
            mutation Refresh($request: RefreshRequest!) {
                refresh(request: $request) {
                    accessToken
                    refreshToken
                    identityToken
                }
            }
        "#,
        "variables": {
            "request": {
                "refreshToken": token_refrescado.to_string()
            }
        }
    });

    let respuesta = cliente
        .post(API_LENS)
        .header("Authorization", format!("Bearer {}", token_autorizado))
        .header("Content-Type", "application/json")
        .json(&consulta)
        .send()
        .await?;

    if respuesta.status().is_success() {
        let json: serde_json::Value = respuesta.json().await?;
        if let Some(autenticacion) = json["data"]["authenticate"].as_object() {
            Ok(LensTokens {
                access_token: autenticacion
                    .get("accessToken")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string(),
                refresh_token: autenticacion
                    .get("refreshToken")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string(),
                identity_token: autenticacion
                    .get("identityToken")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string(),
            })
        } else {
            return Err("Estructura de respuesta inesperada.".into());
        }
    } else {
        return Err(format!("Error: {}", respuesta.status()).into());
    }
}

pub async fn autenticar(
    cliente: Arc<Client>,
    billetera: &LocalWallet,
    perfil_id: &str,
) -> Result<LensTokens, Box<dyn std::error::Error>> {
    let consulta = json!({
        "query": r#"
            query Challenge($request: ChallengeRequest!) {
                challenge(request: $request) {
                    id
                    text
                }
            }
        "#,
        "variables": {
            "request": {
                "signedBy": billetera.address(),
                "for": perfil_id.to_string(),
            }
        }
    });

    let respuesta = cliente
        .post(API_LENS)
        .header("Content-Type", "application/json")
        .json(&consulta)
        .send()
        .await?;

    if respuesta.status().is_success() {
        let json: serde_json::Value = respuesta.json().await?;
        if let Some(desafio) = json["data"]["challenge"].as_object() {
            let firma = billetera
                .sign_message(
                    desafio
                        .get("text")
                        .and_then(|v| v.as_str())
                        .unwrap_or_default()
                        .to_string(),
                )
                .await?;

            let consulta = json!({
                "query": r#"
                        mutation Authenticate($request: SignedAuthChallenge!) {
                            authenticate(request: $request) {
                             accessToken
                             identityToken
                             refreshToken
                            }
                        }
                    "#,
                "variables": {
                    "request": {
                        "id":desafio.get("id")
                        .and_then(|v| v.as_str())
                        .unwrap_or_default()
                        .to_string(),
                        "signature": format!("0x{}", hex::encode(firma.to_vec())),
                    }
                }
            });

            let respuesta = cliente
                .post(API_LENS)
                .header("Content-Type", "application/json")
                .json(&consulta)
                .send()
                .await?;

            if respuesta.status().is_success() {
                let json: serde_json::Value = respuesta.json().await?;
                if let Some(autenticacion) = json["data"]["authenticate"].as_object() {
                    Ok(LensTokens {
                        access_token: autenticacion
                            .get("accessToken")
                            .and_then(|v| v.as_str())
                            .unwrap_or_default()
                            .to_string(),
                        refresh_token: autenticacion
                            .get("refreshToken")
                            .and_then(|v| v.as_str())
                            .unwrap_or_default()
                            .to_string(),
                        identity_token: autenticacion
                            .get("identityToken")
                            .and_then(|v| v.as_str())
                            .unwrap_or_default()
                            .to_string(),
                    })
                } else {
                    return Err("Estructura de respuesta inesperada.".into());
                }
            } else {
                return Err(format!("Error: {}", respuesta.status()).into());
            }
        } else {
            return Err("Estructura de respuesta inesperada.".into());
        }
    } else {
        return Err(format!("Error: {}", respuesta.status()).into());
    }
}


pub fn from_hex_string(hex_str: &str) -> Result<u64, ParseIntError> {
    u64::from_str_radix(hex_str.trim_start_matches("0x"), 16)
}
