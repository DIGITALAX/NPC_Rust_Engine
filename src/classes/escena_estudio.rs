use crate::bib::types::{
    ComandoTrabajador, Escena, EscenaEstudio, Estado, GameTimer, NPCAleatorio, Prohibido,
    RespuestaTrabajadora, Talla, Trabajador,
};
use pathfinding::grid::Grid;
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
        let escena = Arc::new(Mutex::new(None));

        let handle = thread::spawn(move || {
            trabajador_func(cmd_rx, resp_tx);
        });

        let handle_arc = Arc::new(Mutex::new(handle));

        Trabajador {
            sender: cmd_tx,
            receiver,
            handle: Some(handle_arc),
            escena: escena.clone(),
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
        Trabajador::new(|cmd_rx, resp_tx| {
            let escena: Arc<Mutex<Option<Vec<Arc<Mutex<NPCAleatorio>>>>>> =
                Arc::new(Mutex::new(None));
            while let Ok(command) = cmd_rx.recv() {
                match command {
                    ComandoTrabajador::Initialize {
                        sprites,
                        prohibidos,
                        anchura,
                        altura,
                        sillas_ocupadas,
                        sillas,
                    } => {
                        let mut grid = Grid::new(anchura as usize, altura as usize);

                        for area in prohibidos {
                            mark_prohibited(&mut grid, area);
                        }

                        let new_escena = sprites
                            .iter()
                            .map(|sprite| {
                                let npc = NPCAleatorio::new(
                                    sprite.clone(),
                                    Arc::new(Mutex::new(sillas_ocupadas.clone())),
                                    sillas.clone(),
                                    Talla { anchura, altura },
                                    grid.clone(),
                                    GameTimer::new(),
                                );
                                Arc::new(Mutex::new(npc))
                            })
                            .collect();

                        let mut guard = escena.lock().unwrap();
                        *guard = Some(new_escena);
                    }
                    ComandoTrabajador::Start => {
                        if let Some(escena) = &*escena.lock().unwrap() {
                            for sprite in escena.iter() {
                                sprite.lock().unwrap().update(10000);
                            }
                            thread::sleep(Duration::from_millis(10000));
                        }
                    }
                    ComandoTrabajador::RequestState { clave } => {
                        if let Some(escena) = &*escena.lock().unwrap() {
                            let estados: Vec<Vec<Estado>> = escena
                                .iter()
                                .map(|npc| npc.lock().unwrap().get_state().into_iter().collect())
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
        })
    }
}

impl EscenaEstudio {
    pub fn new(escena: Escena, trabajador: Trabajador) -> Self {
        let sprites = escena.sprites.clone();
        let prohibidos = escena.prohibido.clone();
        let anchura = escena.mundo.anchura - (sprites[0].anchura * sprites[0].escala.x) / 2.0;
        let altura = escena.mundo.altura - (sprites[0].altura * sprites[0].escala.y) / 2.0;

        trabajador.post_message(ComandoTrabajador::Initialize {
            sprites: sprites,
            prohibidos: prohibidos,
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

fn mark_prohibited(grid: &mut Grid, area: Prohibido) {
    let start_x = area.x.floor() as usize;
    let start_y = area.y.floor() as usize;
    let end_x = (area.x + area.anchura).ceil() as usize;
    let end_y = (area.y + area.altura).ceil() as usize;

    for x in start_x..end_x {
        for y in start_y..end_y {
            grid.add_vertex((x, y));
        }
    }
}
