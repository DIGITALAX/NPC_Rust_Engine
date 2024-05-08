use warp::Filter;
use warp::ws::{Message, WebSocket};
use std::sync::{Arc, Mutex};
use futures::{StreamExt, SinkExt};
use dotenv::dotenv;
use std::env;
mod lib;
use lib::{constants::*, types::*};


#[tokio::main]
async fn main() {
    dotenv().ok();

    let port: u16 = env::var("PORT")
        .unwrap_or_else(|_| "10000".to_string())
        .parse()
        .expect("Invalid port number");

    let clientes: Clientes = Arc::new(Mutex::new(Vec::new()));

    let ws_route = warp::path("ws")
        .and(warp::ws())
        .and(con_clientes(clientes.clone()))
        .map(|ws: warp::ws::Ws, clientes| ws.on_upgrade(move |socket| manejar_conexion(socket, clientes)));

    let hello = warp::path!("hello" / String)
        .map(|name| format!("Hello, {}!", name));

    let routes = ws_route.or(hello);

    warp::serve(routes)
        .run(([0, 0, 0, 0], port))
        .await;
}

async fn manejar_conexion(ws: WebSocket, clientes: Clientes) {
    let (mut sender, mut receiver) = ws.split();
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();

    {
        let mut clientes = clientes.lock().unwrap();
        clientes.push(tx);
    }

    tokio::spawn(async move {
        while let Some(message) = rx.recv().await {
            let _ = sender.send(message).await;
        }
    });

    while let Some(result) = receiver.next().await {
        match result {
            Ok(msg) => {
                if msg.is_text() {
                    let text = msg.to_str().unwrap();
                    println!("Received: {}", text);

                    let response = format!("Echo: {}", text);

                    let clientes = clientes.lock().unwrap();
                    for client in clientes.iter() {
                        let _ = client.send(Message::text(response.clone()));
                    }
                }
            }
            Err(e) => {
                eprintln!("Error receiving message: {:?}", e);
                break;
            }
        }
    }

    {
        let mut clientes = clientes.lock().unwrap();
        if let Some(pos) = clientes.iter().position(|c| c.is_closed()) {
            clientes.remove(pos);
        }
    }
}

fn con_clientes(clientes: Clientes) -> impl Filter<Extract = (Clientes,), Error = std::convert::Infallible> + Clone {
    warp::any().map(move || clientes.clone())
}
