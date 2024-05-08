mod classes;
mod lib;
use lib::types::*;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc::{self};
use warp::ws::{Message, WebSocket};
use warp::Filter;

impl NPCStudioEngine {
    fn new() -> Self {
        let (tx, mut rx) = mpsc::channel(32);

        let escenas = Arc::new(Mutex::new(HashMap::new()));
        let escenas_clone = escenas.clone();

        tokio::spawn(async move {
            while let Some(command) = rx.recv().await {
                match command {
                    ComandoTrabajador::RequestState { clave, enviador } => {
                        let escenas = escenas_clone.lock().unwrap();
                        if let Some(escena) = escenas.get(&clave) {
                            let response = RespuestaTrabajadora::StateResponse {
                                cmd: String::from("stateResponse"),
                                clave: clave.clone(),
                                estados: vec![vec![Estado {
                                    estado: String::from("idle"),
                                    puntos_de_camino: vec![Coordenada { x: 0, y: 0 }],
                                    duracion: None,
                                    npc_etiqueta: String::from("npc_1"),
                                }]],
                            };

                            let _ = enviador.send(response).await;
                        }
                    }
                }
            }
        });

        NPCStudioEngine {
            escenas,
            enviador: tx,
        }
    }

    async fn handle_ws(self: Arc<Self>, ws: WebSocket) {
        let (tx, mut rx) = ws.split();
        let (response_tx, response_rx) = mpsc::channel(1);

        let sender = self.enviador.clone();
        tokio::spawn(async move {
            while let Some(Ok(msg)) = rx.recv().await {
                if let Ok(text) = msg.to_str() {
                    let msg: HashMap<String, String> = serde_json::from_str(text).unwrap();

                    if let Some(clave) = msg.get("claveEscena") {
                        let _ = sender
                            .send(ComandoTrabajador::RequestState {
                                clave: clave.clone(),
                                enviador: response_tx.clone(),
                            })
                            .await;

                        if let Some(response) = response_rx.recv().await {
                            let response = serde_json::to_string(&response).unwrap();
                            let _ = tx.send(Message::text(response)).await;
                        }
                    }
                }
            }
        });
    }

    fn add_escena(&mut self, escena: Escena) {
        let mut escenas = self.escenas.lock().unwrap();
        escenas.insert(escena.clave.clone(), escena);
    }
}

#[tokio::main]
async fn main() {
    dotenv::dotenv().ok();
    env_logger::init();

    let engine = Arc::new(NPCStudioEngine::new());

    let engine_clone = engine.clone();
    let ws_route = warp::path("ws")
        .and(warp::ws())
        .map(move |ws: warp::ws::Ws| {
            let engine_clone = engine_clone.clone();
            ws.on_upgrade(move |socket| engine_clone.handle_ws(socket))
        });

    warp::serve(ws_route).run(([0, 0, 0, 0], 3030)).await;
}
