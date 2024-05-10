use dotenv::dotenv;
use hyper::{body, Method, Request};
use std::{
    collections::HashMap,
    env,
    net::SocketAddr,
    sync::{Arc, Mutex},
};
use tokio::{
    io::BufReader,
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    spawn, sync,
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
    let render_clave: Arc<Mutex<String>> =
        Arc::new(Mutex::new(std::env::var("RENDER_KEY").expect("Sin Clave")));
    let puerto: u16 = puerto.parse::<u16>().expect("Puerto Inválido");
    let addr: SocketAddr = SocketAddr::from(([127, 0, 0, 1], puerto));
    let oyente: TcpListener = TcpListener::bind(addr).await?;
    println!("El Servidor está escuchando el puerto {}", puerto);

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

    bucle_servidor(oyente, render_clave).await?;

    Ok(())
}

async fn bucle_servidor(
    oyente: TcpListener,
    render_clave: Arc<Mutex<String>>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    loop {
        let (stream, _) = oyente.accept().await?;
        let render_clave_clone = Arc::clone(&render_clave);

        spawn(async move {
            if let Err(err) = maneja_conexion(stream, render_clave_clone).await {
                eprintln!("Error al manejar la conexión: {}", err);
            }
        });
    }
}

async fn maneja_conexion(
    stream: TcpStream,
    render_clave: Arc<Mutex<String>>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let stream_arc: Arc<sync::Mutex<TcpStream>> = Arc::new(tokio::sync::Mutex::new(stream));
    let mut stream_lock: sync::MutexGuard<TcpStream> = stream_arc.lock().await;
    let mut buf_reader: BufReader<&mut TcpStream> = BufReader::new(&mut *stream_lock);
    let mut buffer: Vec<u8> = vec![0; 1024];
    let mut request_text: String = String::new();

    loop {
        match buf_reader.read(&mut buffer).await {
            Ok(0) => break,
            Ok(n) => {
                request_text.push_str(&String::from_utf8_lossy(&buffer[..n]));
                if request_text.contains("\r\n\r\n") {
                    break;
                }
            }
            Err(e) => return Err(Box::new(e)),
        }
    }

    let request: Request<body::Bytes> = Request::builder()
        .method(Method::GET)
        .uri("/")
        .header("Host", "localhost")
        .header("User-Agent", "tokio-http-server")
        .body(body::Bytes::new())
        .unwrap();

    let clave_correcta: bool = request
        .headers()
        .get("X-Render-Key")
        .map_or(false, |value| {
            let clave = render_clave.lock().unwrap();
            let value_str = value.to_str().unwrap_or("");
            value_str == *clave
        });

    let origen_http_valido: bool = request
        .headers()
        .get("Origin")
        .map_or(false, |value| value == "http://ejemplo.com");

    if !clave_correcta || !origen_http_valido {
        return Ok(());
    }

    let stream_arc_clone: Arc<sync::Mutex<TcpStream>> = Arc::clone(&stream_arc);
    let mut stream_lock: sync::MutexGuard<TcpStream> = stream_arc_clone.lock().await;
    loop {
        let mut line = String::new();
        if let Err(_) = buf_reader.read_line(&mut line).await {
            break;
        }
        println!("Cliente envió: {}", line);

        if line.trim() == "datosDeEscena" || line.trim() == "indiceDeEscena" {
            let mut clave_escena = String::new();
            if let Err(err) = buf_reader.read_line(&mut clave_escena).await {
                eprintln!("Error al leer los datos del cliente: {}", err);
                break;
            }
            let clave_escena = clave_escena.trim();

            let guardia: sync::RwLockReadGuard<HashMap<String, EscenaEstudio>> =
                MAPA_ESCENA.read().await;

            if let Some(scene) = guardia.get(clave_escena) {
                if let Some(response) = scene.request_state() {
                    match response {
                        RespuestaTrabajadora::StateResponse { estados, .. } => {
                            if line.trim() == "datosDeEscena" {
                                let serialized_response = serde_json::to_string(&estados)
                                    .unwrap_or_else(|_| String::from("Error de serialización"));

                                if let Err(err) =
                                    stream_lock.write_all(serialized_response.as_bytes()).await
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
                                    .find(|escena| escena.clave == clave_escena);
                                if let Some(scene_info) = scene_info {
                                    let json_response = serde_json::json!({
                                        "estados": estados,
                                        "escena": scene_info,
                                    });
                                    let serialized_response = serde_json::to_string(&json_response)
                                        .unwrap_or_else(|_| String::from("Error de serialización"));

                                    if let Err(err) =
                                        stream_lock.write_all(serialized_response.as_bytes()).await
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
                            if let Err(err) = stream_lock.write_all(mensaje.as_bytes()).await {
                                eprintln!("Error al enviar mensaje de error al cliente: {}", err);
                                break;
                            }
                        }
                    }
                } else {
                    let error_message = "Escena no encontrada";
                    if let Err(err) = stream_lock.write_all(error_message.as_bytes()).await {
                        eprintln!("Error al enviar mensaje de error al cliente: {}", err);
                        break;
                    }
                }
            } else {
                let error_message = "Escena no encontrada";
                if let Err(err) = stream_lock.write_all(error_message.as_bytes()).await {
                    eprintln!("Error al enviar mensaje de error al cliente: {}", err);
                    break;
                }
            }
        } else {
            println!("Evento no reconocido: {}", line);
        }
    }

    Ok(())
}
