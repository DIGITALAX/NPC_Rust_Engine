use bib::constants::*;
use bib::types::*;
use dotenv::dotenv;
use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Request, Response, Server};
use std::env;
use std::io::{Read, Write, BufRead, BufReader};
use std::net::{TcpListener, TcpStream};
use tokio::task;
mod bib;
mod classes;

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

    let nuevas_escenas: Vec<EscenaEstudio> = LISTA_ESCENA
        .iter()
        .map(|escena| EscenaEstudio::new(escena.clone(), Trabajador::default()))
        .collect();

    task::spawn(async move {
        let mut reloj = GameTimer::new();

        loop {
            let delta: u64 = 10000;
            reloj.tick(delta);
            tokio::time::sleep(tokio::time::Duration::from_millis(SUEÑO)).await;
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
                    if let Err(err) = handle_tcp_client(&mut corriente, &render_clave_clone) {
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

fn handle_tcp_client(corriente: &mut TcpStream, render_clave: &str) -> std::io::Result<()> {
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
                    match msg.trim() {
                        "enviarSceneIndex" => {
                            writer.write_all(b"Respuesta1\n")?;
                        }
                        "datosDeEscena" => {
                            writer.write_all(b"Respuesta2\n")?;
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
