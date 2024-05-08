use crate::lib::types::{
    ComandoTrabajador, Escena, EscenaEstudio, RespuestaTrabajadora, Sprite,
    Trabajador,
};
use crate::lib::constants::LISTA_ESCENA;
use std::sync::{mpsc, Arc, Mutex};
use std::thread;

impl Trabajador {
    fn new<F>(worker_func: F) -> Self
    where
        F: FnOnce(mpsc::Receiver<ComandoTrabajador>, mpsc::Sender<RespuestaTrabajadora>)
            + Send
            + 'static,
    {
        let (cmd_tx, cmd_rx) = mpsc::channel();
        let (resp_tx, resp_rx) = mpsc::channel();
        let receiver = Arc::new(Mutex::new(resp_rx));

        let handle = thread::spawn(move || {
            worker_func(cmd_rx, resp_tx);
        });

        let handle_arc = Arc::new(Mutex::new(handle));

        Trabajador {
            sender: cmd_tx,
            receiver,
            handle: Some(handle_arc),
        }
    }

    fn post_message(&self, command: ComandoTrabajador) {
        self.sender
            .send(command)
            .expect("Error enviando mensaje al Worker");
    }

    fn get_response(&self) -> Option<RespuestaTrabajadora> {
        let receiver = self.receiver.lock().unwrap();
        receiver.try_recv().ok()
    }
}

impl Drop for Trabajador {
    fn drop(&mut self) {
        if let Some(handle_arc) = self.handle.take() {
            if let Ok(handle_mutex) = Arc::try_unwrap(handle_arc) {
                if let Ok(mut handle) = handle_mutex.into_inner() {
                    handle.join().expect("Error uniendo Worker");
                }
            }
        }
    }
}

impl Default for Trabajador {
    fn default() -> Self {
        Trabajador::new(|cmd_rx, resp_tx| {
            while let Ok(command) = cmd_rx.recv() {
                match command {
                    ComandoTrabajador::Initialize { clave, .. } => {
                        println!("Worker inicializado con la clave: {}", clave);
                    }
                    ComandoTrabajador::Start => {
                        println!("Worker iniciado");
                    }
                    ComandoTrabajador::RequestState { clave } => {
                        let response = RespuestaTrabajadora::StateResponse {
                            cmd: String::from("stateResponse"),
                            clave,
                            estados: vec![vec![]],
                        };
                        resp_tx.send(response).expect("Error enviando respuesta");
                    }
                }
            }
        })
    }
}


impl EscenaEstudio {
    pub fn new(escena: Escena, trabajador: Trabajador) -> Self {
        let sprites: &Vec<Sprite> = &escena.sprites;

        let anchura = escena.mundo.anchura - (sprites[0].anchura * sprites[0].escala.x) / 2.0;
        let altura = escena.mundo.altura - (sprites[0].altura * sprites[0].escala.y) / 2.0;

        trabajador.post_message(ComandoTrabajador::Initialize {
            sprites: Some(escena.sprites.clone()),
            prohibidos: Some(escena.prohibido.clone()),
            anchura,
            altura,
            clave: escena.clave.clone(),
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

fn main() {
    let escena = &LISTA_ESCENA[0];

    let trabajador = Trabajador::new(|cmd_rx, resp_tx| {
        while let Ok(command) = cmd_rx.recv() {
            match command {
                ComandoTrabajador::Initialize { clave, .. } => {
                    println!("Worker inicializado con la clave: {}", clave);
                }
                ComandoTrabajador::Start => {
                    println!("Worker iniciado");
                }
                ComandoTrabajador::RequestState { clave } => {
                    let response = RespuestaTrabajadora::StateResponse {
                        cmd: String::from("stateResponse"),
                        clave,
                        estados: vec![vec![]],
                    };
                    resp_tx.send(response).expect("Error enviando respuesta");
                }
            }
        }
    });

    let estudio = EscenaEstudio::new(escena.clone(), trabajador);

    if let Some(response) = estudio.request_state() {
        println!("{:?}", response);
    }
}
