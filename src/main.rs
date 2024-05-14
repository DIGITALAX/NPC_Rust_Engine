use dotenv::dotenv;
use futures_util::{future::try_join_all, SinkExt, StreamExt};
use serde_json::{from_str, json, to_string, Value};
use std::{collections::HashMap, env, net::SocketAddr, sync::Arc, time::Duration};
use tokio::{
    net::{TcpListener, TcpStream},
    spawn,
    sync::{
        mpsc::{channel, Receiver, Sender},
        Mutex,
    },
    time::{self},
};
use tokio_tungstenite::{
    accept_hdr_async,
    tungstenite::{
        handshake::server::{ErrorResponse, Request, Response},
        Error, Message,
    },
};
use tungstenite::http::method;

mod bib;
mod classes;
use bib::constants::*;
use bib::types::*;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    dotenv().ok();
    let render_clave = std::env::var("RENDER_KEY").expect("Sin Clave");
    let puerto: String = env::var("PORT").unwrap_or_else(|_| "10000".to_string());
    let puerto: u16 = puerto.parse::<u16>().expect("Puerto Inválido");
    let addr = format!("0.0.0.0:{}", puerto);
    let addr: SocketAddr = addr.parse().expect("Dirección Inválida");
    // let addr: SocketAddr = SocketAddr::from(([127, 0, 0, 1], puerto));
    let oyente = TcpListener::bind(&addr)
        .await
        .expect("No se pudo vincular a la dirección");

    let futures: Vec<_> = LISTA_ESCENA
        .iter()
        .map(|escena: &Escena| async { Ok::<_, Error>(EscenaEstudio::new(escena.clone()).await) })
        .collect();

    let resultados = try_join_all(futures).await.unwrap();

    let mut mapa_escena = HashMap::new();
    for escena in resultados {
        mapa_escena.insert(escena.clave.clone(), escena);
    }
    let (tx, rx) = channel::<HashMap<String, EscenaEstudio>>(100);
    let rx = Arc::new(Mutex::new(rx));
    let mapa_clone = Arc::new(Mutex::new(mapa_escena));

    spawn(async move {
        bucle_juego(mapa_clone, tx).await;
    });

    while let Ok((stream, _)) = oyente.accept().await {
        let render_clone = render_clave.clone();
        let rx_clone = rx.clone();
        spawn(async move {
            if let Err(err) = manejar_conexion(stream, render_clone, rx_clone).await {
                if !err.to_string().contains("Handshake not finished")
                    && !err.to_string().contains("Unsupported HTTP method used")
                {
                    eprintln!("Error al manejar la conexión: {}", err);
                } else {
                    dbg!("Debug: {}", err);
                }
            }
        });
    }

    Ok(())
}

async fn manejar_conexion(
    stream: TcpStream,
    render_clave: String,
    rx: Arc<Mutex<Receiver<HashMap<String, EscenaEstudio>>>>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let ws_stream = accept_hdr_async(stream, |req: &Request, response: Response| {
        if req.method() != method::Method::GET && req.method() != method::Method::HEAD {
            return Ok(response);
        }

        if req.method() == method::Method::GET {
            let uri = req.uri();
            let query: Option<&str> = uri.query();
            let origen: Option<&hyper::header::HeaderValue> = req.headers().get("origin");

            if let Some(query) = query {
                let key_from_client = query.split('=').nth(1);
                if let Some(key) = key_from_client {
                    if key.trim_end_matches("&EIO") == render_clave.trim() {
                        if let Some(origen) = origen {
                            if origen == "https://www.npcstudio.xyz" {
                                Ok(response)
                            } else {
                                Err(ErrorResponse::new(Some("Forbidden".to_string())))
                            }
                        } else {
                            Err(ErrorResponse::new(Some("Forbidden".to_string())))
                        }
                    } else {
                        Err(ErrorResponse::new(Some("Forbidden".to_string())))
                    }
                } else {
                    Err(ErrorResponse::new(Some("Bad Request".to_string())))
                }
            } else {
                Err(ErrorResponse::new(Some("Bad Request".to_string())))
            }
        } else {
            Ok(response)
        }
    })
    .await?;

    let (mut write, mut read) = ws_stream.split();

    while let Some(Ok(msg)) = read.next().await {
        match msg {
            Message::Text(text) => {
                if let Ok(parsed) = from_str::<Value>(&text) {
                    if let Some(tipo) = parsed.get("tipo").and_then(Value::as_str) {
                        if tipo == "datosDeEscena" || tipo == "indiceDeEscena" {
                            if let Some(clave) = parsed.get("clave").and_then(Value::as_str) {
                                let escenas = {
                                    let mut rx = rx.lock().await;
                                    rx.recv().await
                                };

                                if let Some(mut escenas) = escenas {
                                    if let Some(scene) = escenas.get_mut(clave) {
                                        if let Some(response) = scene.request_state() {
                                            match response {
                                                RespuestaTrabajadora::StateResponse {
                                                    estados,
                                                    ..
                                                } => {
                                                    if tipo.trim() == "datosDeEscena" {
                                                        let json_response = json!({
                                                            "nombre": clave,
                                                            "estados": &estados
                                                        });
                                                        let serialized_response =
                                                            to_string(&json_response)
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
                                                    } else {
                                                        let scene_info = LISTA_ESCENA
                                                            .iter()
                                                            .find(|escena| escena.clave == clave);
                                                        if let Some(scene_info) = scene_info {
                                                            let json_response = json!({
                                                                "nombre": "configurarEscena",
                                                                "datos": {
                                                                "estados": estados,
                                                                "escena": scene_info,
                                                                }
                                                            });
                                                            let serialized_response =
                                                                to_string(&json_response)
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
                                                .send(Message::Text(
                                                    "Escena no encontrada".to_string(),
                                                ))
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
                            }
                        } else {
                            println!("Evento no reconocido: {}", tipo);
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

async fn bucle_juego(
    escenas: Arc<Mutex<HashMap<String, EscenaEstudio>>>,
    tx: Sender<HashMap<String, EscenaEstudio>>,
) {
    loop {
        {
            let mut escenas = escenas.lock().await;
            for (_, escena) in escenas.iter_mut() {
                escena.start_loop(1000);
            }
        }
        if let Ok(escenas) = escenas.try_lock() {
            if let Err(err) = tx.send(escenas.clone()).await {
                eprintln!("Error al enviar actualizaciones de escenas: {}", err);
            }
        }

        time::sleep(Duration::from_secs(1)).await;
    }
}
