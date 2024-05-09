use bib::constants::*;
use bib::types::*;
use dotenv::dotenv;
use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Request, Response, Server};
use std::collections::HashMap;
use std::env;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use tokio::{sync, task};
mod bib;
mod classes;

lazy_static::lazy_static! {
    static ref SCENE_MAP: sync::RwLock<HashMap<String, EscenaEstudio>> = sync::RwLock::new(HashMap::new());
}

#[tokio::main]
async fn main() -> std::io::Result<()> {
    dotenv().ok();

    let puerto: String = env::var("PORT").unwrap_or_else(|_| "8080".to_string());
    let puerto = puerto.parse::<u16>().expect("Puerto Invalido");
    let render_clave = env::var("RENDER_KEY").expect("Sin Clave");
    let listener = TcpListener::bind(("0.0.0.0", puerto))?;
    println!("El Servidor está escuchando el puerto {}", puerto);
    let make_svc = make_service_fn(move |_conn| async move {
        Ok::<_, hyper::Error>(service_fn(move |req| handle_request(req)))
    });

    let addr = ([127, 0, 0, 1], puerto).into();
    let server = Server::bind(&addr).serve(make_svc);

    task::spawn(async move {
        let nuevas_escenas: Vec<EscenaEstudio> = LISTA_ESCENA
            .iter()
            .map(|escena| EscenaEstudio::new(escena.clone(), Trabajador::default()))
            .collect();

        let mut scene_map = SCENE_MAP.write().await;

        for escena in nuevas_escenas {
            scene_map.insert(escena.clave.clone(), escena);
        }
    });

    tokio::spawn(async move {
        if let Err(e) = server.await {
            eprintln!("Error en el servidor HTTP: {}", e);
        }
    });

    for corriente in listener.incoming() {
        match corriente {
            Ok(mut corriente) => {
                let render_clave_clone = render_clave.clone();
                task::spawn(async move {
                    if let Err(err) = handle_tcp_client(&mut corriente, &render_clave_clone).await {
                        eprintln!("Error al manejar la conexión del cliente: {}", err);
                    }
                });
            }
            Err(e) => {
                eprintln!("Error: {}", e);
            }
        }
    }

    Ok(())
}

async fn handle_request(req: Request<Body>) -> Result<Response<Body>, hyper::Error> {
    match (req.method(), req.uri().path()) {
        (&hyper::Method::GET, "/") => {
            if let Some(origin) = req.headers().get("Origin") {
                let origin_str = origin.to_str().unwrap_or("");
                if origin_str == "https://npcstudio.xyz" {
                    return Ok(Response::new(Body::from(
                        "Conexion de WebSocket establecida",
                    )));
                } else {
                    eprintln!(
                        "Intento de conexión WebSocket desde un origen no permitido: {}",
                        origin_str
                    );
                    let mut res = Response::default();
                    *res.status_mut() = hyper::StatusCode::UNAUTHORIZED;
                    return Ok(res);
                }
            } else {
                let mut res = Response::default();
                *res.status_mut() = hyper::StatusCode::BAD_REQUEST;
                return Ok(res);
            }
        }
        _ => {
            let mut res = Response::default();
            *res.status_mut() = hyper::StatusCode::NOT_FOUND;
            Ok(res)
        }
    }
}

async fn handle_tcp_client(corriente: &mut TcpStream, render_clave: &str) -> std::io::Result<()> {
    let mut buffer = [0; 1024];
    corriente.read(&mut buffer)?;

    let clave_recibida = String::from_utf8_lossy(&buffer[..]);
    if clave_recibida.trim() == render_clave {
        corriente.write_all(b"Conexion establecida!\n")?;
        println!("Cliente Se Conectó.");

        let corriente_clone = corriente.try_clone()?;
        let mut reader = BufReader::new(corriente);
        let mut writer = corriente_clone;

        loop {
            let mut msg = String::new();
            match reader.read_line(&mut msg) {
                Ok(0) => {
                    println!("Cliente desconectado.");
                    return Ok(());
                }
                Ok(_) => {
                    println!("Mensaje recibido del cliente: {}", msg);
                    let parts: Vec<&str> = msg.trim().split("+").collect();
                    if parts.len() != 2 {
                        writer.write_all(b"Mensaje no reconocido\n")?;
                        writer.flush()?;
                        continue;
                    }
                    let command = parts[0];
                    let scene_key = parts[1];
                    match command {
                        "indiceDeEscena" => {
                            let scene_guard = SCENE_MAP.read().await;
                            if let Some(scene) = scene_guard.get(scene_key) {
                                if let Some(response) = scene.request_state() {
                                    match response {
                                        RespuestaTrabajadora::StateResponse { estados, .. } => {
                                            let scene_info = LISTA_ESCENA
                                                .iter()
                                                .find(|escena| escena.clave == scene_key);
                                            if let Some(scene_info) = scene_info {
                                                let json_response = serde_json::json!({
                                                    "estados": estados,
                                                    "escena": scene_info,
                                                });
                                                let serialized_response =
                                                    serde_json::to_string(&json_response)
                                                        .unwrap_or_else(|_| {
                                                            String::from("Error de serialización")
                                                        });
                                                writer.write_all(serialized_response.as_bytes())?;
                                            } else {
                                                writer.write_all(b"Escena no encontrada\n")?;
                                            }
                                        }
                                        RespuestaTrabajadora::Error { mensaje } => {
                                            writer.write_all(
                                                format!("Error: {}", mensaje).as_bytes(),
                                            )?;
                                        }
                                    }
                                } else {
                                    writer.write_all(b"Error obteniendo estado de la escena\n")?;
                                }
                            } else {
                                writer.write_all(b"Escena no encontrada\n")?;
                            }
                        }
                        "datosDeEscena" => {
                            let scene_guard = SCENE_MAP.read().await;
                            if let Some(scene) = scene_guard.get(scene_key) {
                                if let Some(response) = scene.request_state() {
                                    match response {
                                        RespuestaTrabajadora::StateResponse { estados, .. } => {
                                            let serialized_response =
                                                serde_json::to_string(&estados).unwrap_or_else(
                                                    |_| String::from("Error de serialización"),
                                                );
                                            writer.write_all(serialized_response.as_bytes())?;
                                        }
                                        RespuestaTrabajadora::Error { mensaje } => {
                                            writer.write_all(
                                                format!("Error: {}", mensaje).as_bytes(),
                                            )?;
                                        }
                                    }
                                } else {
                                    writer.write_all(b"Error obteniendo estado de la escena\n")?;
                                }
                            } else {
                                writer.write_all(b"Escena no encontrada\n")?;
                            }
                        }
                        _ => {
                            writer.write_all(b"Mensaje no reconocido\n")?;
                        }
                    }
                    writer.flush()?;
                }
                Err(e) => {
                    eprintln!("Error al leer del cliente: {}", e);
                    return Err(e);
                }
            }
        }
    } else {
        corriente.write_all(b"Clave Invalida!")?;
        println!("Conexion al Cliente Rechazada.");
    }

    Ok(())
}
