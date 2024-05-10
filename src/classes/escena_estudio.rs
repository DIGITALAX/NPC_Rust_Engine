use crate::bib::types::{
    ComandoTrabajador, Escena, EscenaEstudio, Estado, GameTimer, NPCAleatorio, Prohibido,
    RespuestaTrabajadora, Talla, Trabajador,
};
use pathfinding::map::MapManager;
use std::cmp::min;
use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use tokio::time::Duration;

impl Trabajador {
    fn new<F>(trabajador_func: F) -> Self
    where
        F: FnOnce(mpsc::Receiver<ComandoTrabajador>, mpsc::Sender<RespuestaTrabajadora>)
            + Send
            + 'static,
    {
        let (cmd_tx, cmd_rx) = mpsc::channel();
        let (resp_tx, resp_rx) = mpsc::channel();
        let receiver = Arc::new(Mutex::new(resp_rx));
        let escena = Arc::new(Mutex::new(Some(Vec::new())));

        let handle = thread::spawn(move || {
            trabajador_func(cmd_rx, resp_tx);
        });

        let handle_arc = Arc::new(Mutex::new(handle));

        Trabajador {
            sender: cmd_tx,
            receiver,
            handle: Some(handle_arc),
            escena,
        }
    }

    fn post_message(&self, command: ComandoTrabajador) {
        self.sender
            .send(command)
            .expect("Error enviando mensaje al Trabajador");
    }

    fn get_response(&self) -> Option<RespuestaTrabajadora> {
        let receiver = self.receiver.lock().unwrap();
        receiver.try_recv().ok()
    }
}

impl Default for Trabajador {
    fn default() -> Self {
        let (cmd_tx, cmd_rx) = mpsc::channel();
        let (resp_tx, resp_rx) = mpsc::channel();
        let receiver = Arc::new(Mutex::new(resp_rx));
        let escena = Arc::new(Mutex::new(Some(Vec::new())));

        let handle = thread::spawn(move || {
            let runtime = tokio::runtime::Builder::new_multi_thread()
                .worker_threads(2)
                .enable_all()
                .build()
                .expect("Error creando el runtime");

            let escena: Arc<Mutex<Option<Vec<Arc<Mutex<NPCAleatorio>>>>>> =
                Arc::new(Mutex::new(Some(Vec::new())));

            runtime.block_on(async move {
                while let Ok(command) = cmd_rx.recv() {
                    let escena_clone = escena.clone();
                    match command {
                        ComandoTrabajador::Initialize {
                            sprites,
                            prohibidos,
                            anchura,
                            altura,
                            sillas_ocupadas,
                            sillas,
                        } => {
                            let mapa = MapManager::get_instance();
                            let mut mapa_guard = mapa.write().await;
                            let mapa_id = mapa_guard.new_astar().await;
                            let mapa_matriz =
                                create_map(anchura as usize, altura as usize, prohibidos);

                            if let Err(err) = mapa_guard.load(&mapa_id, mapa_matriz).await {
                                println!("Error al cargar el mapa: {}", err);
                                continue;
                            }

                            let new_escena: Vec<Arc<Mutex<NPCAleatorio>>> = sprites
                                .iter()
                                .map(|sprite| {
                                    let npc = NPCAleatorio::new(
                                        sprite.clone(),
                                        Arc::new(Mutex::new(sillas_ocupadas.clone())),
                                        sillas.clone(),
                                        Talla { anchura, altura },
                                        mapa_id,
                                        mapa.clone(),
                                        GameTimer::new(),
                                    );
                                    Arc::new(Mutex::new(npc))
                                })
                                .collect();

                            let new_escena_arc = Arc::new(Mutex::new(new_escena));

                            let mut guard = escena_clone.lock().unwrap();
                            *guard = Some(new_escena_arc.lock().unwrap().clone());
                        }
                        ComandoTrabajador::Start => {
                            if let Some(escena) = escena.lock().unwrap().as_ref() {
                                for sprite in escena.iter() {
                                    sprite.lock().unwrap().update(10000);
                                }
                                tokio::time::sleep(Duration::from_millis(10000)).await;
                            }
                        }
                        ComandoTrabajador::RequestState { clave } => {
                            if let Some(escena) = escena.lock().unwrap().as_ref() {
                                let estados: Vec<Vec<Estado>> = escena
                                    .iter()
                                    .map(|npc| {
                                        npc.lock().unwrap().get_state().into_iter().collect()
                                    })
                                    .collect();
                                let response = RespuestaTrabajadora::StateResponse {
                                    cmd: String::from("stateResponse"),
                                    clave,
                                    estados,
                                };
                                resp_tx.send(response).expect("Error enviando respuesta");
                            }
                        }
                    }
                }
            });
        });

        let handle_arc = Arc::new(Mutex::new(handle));

        Trabajador {
            sender: cmd_tx,
            receiver,
            handle: Some(handle_arc),
            escena,
        }
    }
}

impl EscenaEstudio {
    pub fn new(escena: Escena, trabajador: Trabajador) -> Self {
        let sprites = escena.sprites.clone();
        let prohibidos = escena.prohibido.clone();
        let anchura = escena.mundo.anchura - (sprites[0].anchura * sprites[0].escala.x) / 2.0;
        let altura = escena.mundo.altura - (sprites[0].altura * sprites[0].escala.y) / 2.0;

        trabajador.post_message(ComandoTrabajador::Initialize {
            sprites,
            prohibidos,
            anchura,
            altura,
            sillas_ocupadas: Vec::new(),
            sillas: escena.sillas.clone(),
        });

        trabajador.post_message(ComandoTrabajador::Start);

        EscenaEstudio {
            clave: escena.clave,
            sillas_ocupadas: Vec::new(),
            trabajador,
        }
    }

    pub fn request_state(&self) -> Option<RespuestaTrabajadora> {
        self.trabajador
            .post_message(ComandoTrabajador::RequestState {
                clave: self.clave.clone(),
            });
        self.trabajador.get_response()
    }
}

fn create_map(
    anchura_mundo: usize,
    altura_mundo: usize,
    prohibidos: Vec<Prohibido>,
) -> Vec<Vec<i32>> {
    let mut map = vec![vec![0; anchura_mundo]; altura_mundo];

    for prohibido in prohibidos {
        let start_x = prohibido.x.floor() as usize;
        let start_y = prohibido.y.floor() as usize;
        let end_x = min(
            (prohibido.x + prohibido.anchura).ceil() as usize,
            anchura_mundo,
        );
        let end_y = min(
            (prohibido.y + prohibido.altura).ceil() as usize,
            altura_mundo,
        );

        for y in start_y..end_y {
            for x in start_x..end_x {
                map[y][x] = 1;
            }
        }
    }

    map
}
