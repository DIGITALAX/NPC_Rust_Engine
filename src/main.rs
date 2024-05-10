use dotenv::dotenv;
use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use std::{collections::HashMap, env, net::SocketAddr};
use tokio::{
    net::{TcpListener, TcpStream},
    spawn, sync,
};
use tokio_tungstenite::{
    accept_hdr_async,
    tungstenite::{
        handshake::server::{ErrorResponse, Request, Response},
        Message,
    },
};

mod bib;
mod classes;
use bib::constants::*;
use bib::types::*;

lazy_static::lazy_static! {
    static ref MAPA_ESCENA: sync::RwLock<HashMap<String, EscenaEstudio>> = sync::RwLock::new(HashMap::new());
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    dotenv().ok();

    let puerto: String = env::var("PORT").unwrap_or_else(|_| "8080".to_string());
    let puerto: u16 = puerto.parse::<u16>().expect("Puerto Inválido");
    let addr: SocketAddr = SocketAddr::from(([127, 0, 0, 1], puerto));
    let oyente = TcpListener::bind(&addr)
        .await
        .expect("No se pudo vincular a la dirección");

    spawn(async move {
        let nuevas_escenas: Vec<EscenaEstudio> = LISTA_ESCENA
            .iter()
            .map(|escena: &Escena| EscenaEstudio::new(escena.clone(), Trabajador::default()))
            .collect();

        let mut mapa_escena: sync::RwLockWriteGuard<HashMap<String, EscenaEstudio>> =
            MAPA_ESCENA.write().await;

        for escena in nuevas_escenas {
            mapa_escena.insert(escena.clave.clone(), escena);
        }
    });

    while let Ok((stream, _)) = oyente.accept().await {
        tokio::spawn(async move {
            if let Err(err) = manejar_conexion(stream).await {
                eprintln!("Error al manejar la conexión: {}", err);
            }
        });
    }

    Ok(())
}

async fn manejar_conexion(
    stream: TcpStream,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let render_clave = std::env::var("RENDER_KEY").expect("Sin Clave");
    let ws_stream = accept_hdr_async(stream, |req: &Request, response: Response| {
        let uri = req.uri();
        let query: Option<&str> = uri.query();
        let origen: Option<&hyper::header::HeaderValue> = req.headers().get("origin");

        if let Some(query) = query {
            let key_from_client = query.split('=').nth(1);

            if let Some(key) = key_from_client {
                if key.trim_end_matches("&EIO") == render_clave.trim() {
                    // if let Some(origen) = origen {
                    // if origen == "https://npcstudio.xyz" {
                    Ok(response)
                    // } else {
                    //     Err(ErrorResponse::new(Some("Forbidden".to_string())))
                    // }
                    // } else {
                    //     Err(ErrorResponse::new(Some("Forbidden".to_string())))
                    // }
                } else {
                    Err(ErrorResponse::new(Some("Forbidden".to_string())))
                }
            } else {
                Err(ErrorResponse::new(Some("Bad Request".to_string())))
            }
        } else {
            Err(ErrorResponse::new(Some("Bad Request".to_string())))
        }
    })
    .await?;

    let (mut write, mut read) = ws_stream.split();

    write
        .send(Message::Text(format!(
            "{{ \"{}\": {} }}",
            "here",
            json!({})
        )))
        .await
        .expect("Error sending connect message");

    while let Some(Ok(msg)) = read.next().await {
        match msg {
            Message::Text(text) => {
                if let Ok(parsed) = serde_json::from_str::<Value>(&text) {
                    if let Some(tipo) = parsed.get("type").and_then(Value::as_str) {
                        if tipo == "datosDeEscena" || tipo == "indiceDeEscena" {
                            if let Some(clave) = parsed.get("sceneKey").and_then(Value::as_str) {
                                let guardia: sync::RwLockReadGuard<HashMap<String, EscenaEstudio>> =
                                    MAPA_ESCENA.read().await;

                                if let Some(scene) = guardia.get(clave) {
                                    if let Some(response) = scene.request_state() {
                                        match response {
                                            RespuestaTrabajadora::StateResponse {
                                                estados, ..
                                            } => {
                                                if text.trim() == "datosDeEscena" {
                                                    let serialized_response =
                                                        serde_json::to_string(&estados)
                                                            .unwrap_or_else(|_| {
                                                                String::from(
                                                                    "Error de serialización",
                                                                )
                                                            });

                                                    if let Err(err) = write
                                                        .send(Message::Text(serialized_response))
                                                        .await
                                                    {
                                                        eprintln!(
                                                "Error al enviar la respuesta de estado de la escena: {}",
                                                err
                                            );
                                                        break;
                                                    }
                                                } else {
                                                    let scene_info = LISTA_ESCENA
                                                        .iter()
                                                        .find(|escena| escena.clave == clave);
                                                    if let Some(scene_info) = scene_info {
                                                        let json_response = serde_json::json!({
                                                            "nombre": "configurarEscena",
                                                            "datos": {
                                                            "estados": estados,
                                                            "escena": scene_info,
                                                            }
                                                        });
                                                        let serialized_response =
                                                            serde_json::to_string(&json_response)
                                                                .unwrap_or_else(|_| {
                                                                    String::from(
                                                                        "Error de serialización",
                                                                    )
                                                                });

                                                        if let Err(err) = write
                                                            .send(Message::Text(
                                                                serialized_response,
                                                            ))
                                                            .await
                                                        {
                                                            eprintln!(
                                                    "Error al enviar la respuesta de estado de la escena: {}",
                                                    err
                                                );
                                                            break;
                                                        }
                                                    }
                                                }
                                            }
                                            RespuestaTrabajadora::Error { mensaje } => {
                                                if let Err(err) = write
                                                    .send(Message::Text(mensaje.to_string()))
                                                    .await
                                                {
                                                    eprintln!(
                                                "Error al enviar mensaje de error al cliente: {}",
                                                err
                                            );
                                                    break;
                                                }
                                            }
                                        }
                                    } else {
                                        if let Err(err) = write
                                            .send(Message::Text("Escena no encontrada".to_string()))
                                            .await
                                        {
                                            eprintln!(
                                                "Error al enviar mensaje de error al cliente: {}",
                                                err
                                            );
                                            break;
                                        }
                                    }
                                }
                            }
                        } else {
                            println!("Evento no reconocido: {}", text);
                        }
                    }
                }
            }
            _ => {
                println!("Tipo de mensaje no soportado: {:?}", msg);
            }
        }
    }

    Ok(())
}
