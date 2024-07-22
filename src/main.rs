use dotenv::dotenv;
use futures_util::{future::try_join_all, SinkExt, StreamExt};
use serde_json::{from_str, json, to_string, Value};
use std::{collections::HashMap, env, net::SocketAddr, sync::Arc, time::Duration};
use tokio::{
    net::{TcpListener, TcpStream},
    spawn,
    sync::RwLock,
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
    let oyente = TcpListener::bind(&addr)
        .await
        .expect("No se pudo vincular a la dirección");
    let rt = tokio::runtime::Runtime::new().unwrap();
    let manija = rt.handle().clone();

    let futures: Vec<_> = LISTA_ESCENA
        .iter()
        .map(|escena: &Escena| async {
            Ok::<_, Error>(EscenaEstudio::new(escena.clone(), manija.clone()).await)
        })
        .collect();

    let resultados = try_join_all(futures).await.unwrap();

    let mut mapa_escena = HashMap::new();
    for escena in resultados {
        mapa_escena.insert(escena.clave.clone(), escena);
    }
    let mapa_escena = Arc::new(RwLock::new(mapa_escena));

    let mapa_escena_clone = mapa_escena.clone();
    spawn(async move {
        bucle_juego(mapa_escena_clone).await;
    });

    while let Ok((stream, _)) = oyente.accept().await {
        let render_clone = render_clave.clone();
        let mapa_escena_clone = mapa_escena.clone();
        spawn(async move {
            if let Err(err) = manejar_conexion(stream, render_clone, mapa_escena_clone).await {
                if !err.to_string().contains("Handshake not finished")
                    && !err.to_string().contains("Unsupported HTTP method used")
                {
                    eprintln!("Error al manejar la conexión: {}", err);
                } else {
                    eprintln!("Debug: {}", err);
                }
            }
        });
    }

    Ok(())
}

async fn manejar_conexion(
    stream: TcpStream,
    render_clave: String,
    escenas: Arc<RwLock<HashMap<String, EscenaEstudio>>>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let ws_stream = accept_hdr_async(stream, |req: &Request, respuesta: Response| {
        if req.method() != method::Method::GET && req.method() != method::Method::HEAD {
            return Ok(respuesta);
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
                            if origen == "https://www.npcstudio.xyz"
                                || origen == "https://npc.digitalax.xyz"
                            {
                                Ok(respuesta)
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
            Ok(respuesta)
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
                                let mut escenas_guard = escenas.write().await;

                                if let Some(escena) = escenas_guard.get_mut(clave) {
                                    if let Some(respuesta) = escena.request_state() {
                                        match respuesta {
                                            RespuestaTrabajadora::StateResponse {
                                                estados, ..
                                            } => {
                                                if tipo.trim() == "datosDeEscena" {
                                                    let json_respuesta = json!({
                                                        "nombre": clave,
                                                        "estados": &estados
                                                    });
                                                    let serialized_respuesta = to_string(
                                                        &json_respuesta,
                                                    )
                                                    .unwrap_or_else(|_| {
                                                        String::from("Error de serialización")
                                                    });

                                                    if let Err(err) = write
                                                        .send(Message::Text(serialized_respuesta))
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
                                                        let json_respuesta = json!({
                                                            "nombre": "configurarEscena",
                                                            "datos": {
                                                            "estados": estados,
                                                            "escena": scene_info,
                                                            "todoInfo": LISTA_ESCENA.clone()
                                                            }
                                                        });
                                                        let serialized_respuesta =
                                                            to_string(&json_respuesta)
                                                                .unwrap_or_else(|_| {
                                                                    String::from(
                                                                        "Error de serialización",
                                                                    )
                                                                });

                                                        if let Err(err) = write
                                                            .send(Message::Text(
                                                                serialized_respuesta,
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
                            eprintln!("Evento no reconocido: {}", tipo);
                        }
                    }
                }
            }
            _ => {
                eprintln!("Tipo de mensaje no soportado: {:?}", msg);
            }
        }
    }

    Ok(())
}

async fn bucle_juego(escenas: Arc<RwLock<HashMap<String, EscenaEstudio>>>) {
    loop {
        let escenas_clonadas: HashMap<_, _>;
        {
            let escenas_guard = escenas.read().await;
            escenas_clonadas = escenas_guard.clone();
        }

        let mut escenas_actualizadas = HashMap::new();
        for (clave, mut escena) in escenas_clonadas {
            escena.ejecutar_bucle(1000);
            escenas_actualizadas.insert(clave, escena);
        }

        {
            let mut escenas_guard = escenas.write().await;
            *escenas_guard = escenas_actualizadas;
        }

        time::sleep(Duration::from_secs(1)).await;
    }
}
