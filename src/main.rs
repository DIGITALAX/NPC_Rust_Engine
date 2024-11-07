use chrono::Utc;
use dotenv::dotenv;
use futures_util::{future::try_join_all, lock::Mutex, SinkExt, StreamExt};
use rand::{rngs::StdRng, Rng, SeedableRng};
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
            return Err(ErrorResponse::new(Some(
                "Método HTTP no soportado".to_string(),
            )));
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
                            match origen.to_str() {
                                Ok(origen_str) => {
                                    if origen_str == "https://www.npcstudio.xyz"
                                        || origen_str == "https://npc.digitalax.xyz"
                                        || origen_str
                                            == "https://glorious-eft-deeply.ngrok-free.app"
                                        || origen_str == "https://npcstudio.xyz"
                                    {
                                        return Ok(respuesta);
                                    } else {
                                        return Err(ErrorResponse::new(Some(
                                            "Forbidden".to_string(),
                                        )));
                                    }
                                }
                                Err(e) => {
                                    eprintln!("Error al procesar el encabezado origin: {:?}", e);
                                    Err(ErrorResponse::new(Some(
                                        "Invalid origin header".to_string(),
                                    )))
                                }
                            }
                        } else {
                            return Err(ErrorResponse::new(Some("Forbidden".to_string())));
                        }
                    } else {
                        return Err(ErrorResponse::new(Some("Forbidden".to_string())));
                    }
                } else {
                    return Err(ErrorResponse::new(Some("Bad Request".to_string())));
                }
            } else {
                return Err(ErrorResponse::new(Some("Bad Request".to_string())));
            }
        } else {
            return Ok(respuesta);
        }
    })
    .await?;

    let (mut write, mut read) = ws_stream.split();

    while let Some(Ok(msg)) = read.next().await {
        match msg {
            Message::Text(text) => {
                if let Ok(parsed) = from_str::<Value>(&text) {
                    if let Some(tipo) = parsed.get("tipo").and_then(Value::as_str) {
                        if tipo == "datosDeEscena"
                            || tipo == "indiceDeEscena"
                            || tipo == "datosDeEscenas"
                            || tipo == "llamaContenido"
                        {
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
                                                } else if tipo.trim() == "datosDeEscenas" {
                                                    let json_respuesta = json!({
                                                        "nombre": "datosDeEscenas",
                                                        "datos": LISTA_ESCENA.clone()
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
                                                    "Error al enviar la respuesta de los datos de las escenas: {}",
                                                    err
                                                );
                                                        break;
                                                    }
                                                } else if tipo == "llamaContenido" {
                                                    if let Some(npc_id) =
                                                        parsed.get("npc").and_then(Value::as_str)
                                                    {
                                                        if let Some(npc) = escena
                                                            .npcs
                                                            .iter_mut()
                                                            .find(|n| n.npc.etiqueta == npc_id)
                                                        {
                                                            if let Some(json) = parsed.get("json") {
                                                                if let Ok(json_string) =
                                                                    serde_json::to_string(json)
                                                                {
                                                                    npc.llama_recibido(
                                                                        &json_string,
                                                                    );
                                                                } else {
                                                                    eprintln!("Error al convertir el contenido JSON a cadena");
                                                                }
                                                            } else {
                                                                if let Err(err) = write
                                                                    .send(Message::Text(
                                                                        "JSON no encontrada"
                                                                            .to_string(),
                                                                    ))
                                                                    .await
                                                                {
                                                                    eprintln!("Error al procesar mensaje de Llama: {}", err);
                                                                    break;
                                                                }
                                                            }
                                                        } else {
                                                            if let Err(err) = write
                                                                .send(Message::Text(
                                                                    "NPC no encontrado".to_string(),
                                                                ))
                                                                .await
                                                            {
                                                                eprintln!("Error al enviar mensaje de NPC no encontrado: {}", err);
                                                                break;
                                                            }
                                                        }
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
    let conteo_alquiler: Arc<Mutex<HashMap<usize, u32>>> = Arc::new(Mutex::new(HashMap::new()));
    let mut ultima_seleccion: chrono::DateTime<Utc> = Utc::now();
    loop {
        let escenas_clonadas: HashMap<_, _>;
        {
            let escenas_guard = escenas.read().await;
            escenas_clonadas = escenas_guard.clone();
        }
        let mut npc_seleccionado = None;

        if Utc::now() - ultima_seleccion >= chrono::Duration::weeks(1) {
            let mut todos_npcs: Vec<NPCAleatorio> = Vec::new();

            for escena in escenas_clonadas.values() {
                todos_npcs.extend(escena.npcs.clone());
            }

            npc_seleccionado = seleccionar_npc(Arc::clone(&conteo_alquiler), &todos_npcs).await;

            if let Some(npc_nombre) = &npc_seleccionado {
                if let Some((indice_npc, _)) = todos_npcs
                    .iter()
                    .enumerate()
                    .find(|(_, npc)| &npc.npc.etiqueta == npc_nombre)
                {
                    incrementar_conteo_alquiler(Arc::clone(&conteo_alquiler), indice_npc).await;
                }
            }
            ultima_seleccion = Utc::now();
        }

        let mut escenas_actualizadas = HashMap::new();

        for (clave, mut escena) in escenas_clonadas {
            escena.ejecutar_bucle(1000, npc_seleccionado.clone());
            escenas_actualizadas.insert(clave, escena);
        }
        {
            let mut escenas_guard = escenas.write().await;
            *escenas_guard = escenas_actualizadas;
        }

        time::sleep(Duration::from_secs(1)).await;
    }
}

async fn seleccionar_npc(
    conteo_alquiler: Arc<Mutex<HashMap<usize, u32>>>,
    npcs: &[NPCAleatorio],
) -> Option<String> {
    let mut rng = StdRng::from_entropy();

    let conteo_guard = conteo_alquiler.lock().await;

    let total_peso: f32 = npcs
        .iter()
        .enumerate()
        .map(|(i, _)| {
            let conteo = *conteo_guard.get(&i).unwrap_or(&0);
            1.0 / (conteo as f32 + 1.0)
        })
        .sum();

    let mut seleccion = rng.gen_range(0.0..total_peso);

    for (i, npc) in npcs.iter().enumerate() {
        let peso_actual = 1.0 / (*conteo_guard.get(&i).unwrap_or(&0) as f32 + 1.0);
        if seleccion < peso_actual {
            return Some(npc.npc.etiqueta.clone());
        }
        seleccion -= peso_actual;
    }

    None
}

async fn incrementar_conteo_alquiler(
    conteo_alquiler: Arc<Mutex<HashMap<usize, u32>>>,
    index: usize,
) {
    let mut conteo_guard = conteo_alquiler.lock().await;
    let contador = conteo_guard.entry(index).or_insert(0);
    *contador += 1;
}
