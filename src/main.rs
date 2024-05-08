use crate::lib::constants::*;
use crate::lib::types::*;
use dotenv::dotenv;
use env_logger;
use futures::{SinkExt, StreamExt};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc::{self, Receiver, Sender};
use warp::ws::{Message, WebSocket};
use warp::Filter;

mod classes;
mod lib;

impl NPCStudioEngine {
    fn new() -> Self {
        let (tx, mut rx) = mpsc::channel(32);
        let escenas = Arc::new(Mutex::new(HashMap::<String, EscenaEstudio>::new()));
        let escenas_clone = Arc::clone(&escenas);
        tokio::spawn(async move {
            while let Some(command) = rx.recv().await {
                let (response_tx, mut response_rx): (
                    Sender<RespuestaTrabajadora>,
                    Receiver<RespuestaTrabajadora>,
                ) = mpsc::channel(1);
        
                match command {
                    ComandoTrabajador::RequestState { clave } => {
                        let escenas = escenas_clone.lock().unwrap().clone();
                        if let Some(escena_estudio) = escenas.get(&clave) {
                            let estados: Vec<Estado> = match escena_estudio.request_state() {
                                Some(RespuestaTrabajadora::StateResponse { estados, .. }) => {
                                    estados
                                        .into_iter()
                                        .flat_map(|inner_vec| inner_vec)
                                        .collect()
                                }
                                _ => vec![],
                            };
                            let estados: Vec<Vec<Estado>> = vec![estados];
                            let response = RespuestaTrabajadora::StateResponse {
                                cmd: String::from("stateResponse"),
                                clave: clave.clone(),
                                estados,
                            };
                            let _ = response_tx.send(response).await;
                        }
                    }
                    ComandoTrabajador::Initialize { .. } => {
                        let mut escenas = escenas_clone.lock().unwrap().clone();
                        let lista_escena: &[Escena; 1] = &*LISTA_ESCENA;
        
                        for escena in lista_escena.iter() {
                            escenas.insert(
                                escena.clave.clone(),
                                EscenaEstudio::new(escena.clone(), Default::default()),
                            );
                        }
        
                        let mut escenas_guard = escenas_clone.lock().unwrap();
                        *escenas_guard = escenas;
                    }
        
                    ComandoTrabajador::Start => {}
                }
            }
        });

        NPCStudioEngine {
            escenas,
            enviador: tx,
        }
    }

    async fn handle_ws(self: Arc<Self>, ws: WebSocket) {
        let (mut tx, mut rx) = ws.split();
        let (_, mut response_rx): (Sender<RespuestaTrabajadora>, Receiver<RespuestaTrabajadora>) =
            mpsc::channel(1);

        let sender = self.enviador.clone();
        tokio::spawn(async move {
            while let Some(Ok(msg)) = rx.next().await {
                if let Ok(text) = msg.to_str() {
                    let msg: HashMap<String, String> = serde_json::from_str(text).unwrap();

                    if let Some(clave) = msg.get("claveEscena") {
                        let _ = sender
                            .send(ComandoTrabajador::RequestState {
                                clave: clave.clone(),
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
        escenas.insert(
            escena.clave.clone(),
            EscenaEstudio::new(escena, Default::default()),
        );
    }

    fn initialize_escenas(&self) {
        let sender = self.enviador.clone();
        tokio::spawn(async move {
            let _ = sender
                .send(ComandoTrabajador::Initialize {
                    sprites: None,
                    prohibidos: None,
                    anchura: 0.0,
                    altura: 0.0,
                    clave: String::from("default"),
                    sillas_ocupadas: vec![],
                    sillas: vec![],
                })
                .await;
        });
    }
}

#[tokio::main]
async fn main() {
    dotenv().ok();
    env_logger::init();
    let render_key = std::env::var("RENDER_KEY").expect("RENDER_KEY no encontrado en .env");

    let engine = Arc::new(NPCStudioEngine::new());
    engine.initialize_escenas();

    let engine_clone = engine.clone();
    let ws_route = warp::path("ws")
        .and(warp::ws())
        .map(move |ws: warp::ws::Ws| {
            let engine_clone = engine_clone.clone();
            ws.on_upgrade(move |socket| engine_clone.handle_ws(socket))
        });

    warp::serve(ws_route).run(([0, 0, 0, 0], 3030)).await;
}
