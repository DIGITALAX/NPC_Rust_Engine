use crate::{
    bib::{
        types::{Coordenada, Estado, GameTimer, Movimiento, NPCAleatorio, Silla, Sprite, Talla},
        utils::between,
    },
    Mapa,
};
use pathfinding::prelude::astar;
use std::sync::{Arc, Mutex};

impl NPCAleatorio {
    pub fn new(
        sprite: Sprite,
        sillas_ocupadas: Arc<Mutex<Vec<Silla>>>,
        sillas: Vec<Silla>,
        mundo: Talla,
        reloj_juego: GameTimer,
        mapa: Mapa,
    ) -> Self {
        NPCAleatorio {
            reloj_juego,
            sillas_ocupadas,
            sillas,
            mundo,
            movimientos_max: sprite.movimientos_max,
            caminos: Vec::new(),
            npc: sprite,
            mapa,
            contador: 0.0,
            silla_cerca: None,
        }
    }

    pub fn get_state(&self) -> &Vec<Estado> {
        &self.caminos
    }

    pub fn update(&mut self, delta_time: u64) {
        self.reloj_juego.tick(delta_time);
        self.set_random_direction();
        self.clean_old_paths();
    }

    fn set_random_direction(&mut self) {
        if self.contador >= self.movimientos_max {
            let sillas_taken = self.sillas_ocupadas.lock().unwrap().len();
            let sillas_total = self.sillas.len();
            let probabilidad_sit = if sillas_taken < sillas_total {
                0.5
            } else {
                0.0
            };
            let ajuste_probabilidad = sillas_taken as f32 / sillas_total as f32;
            let probabilidad_final_sit = probabilidad_sit * (1.0 - ajuste_probabilidad);
            let decision: f32 = rand::random();

            if decision < probabilidad_final_sit {
                self.go_sit();
            } else {
                self.go_idle();
            }
        } else {
            self.go_move();
        }
    }

    fn go_idle(&mut self) {
        self.caminos.push(Estado {
            estado: Movimiento::Idle,
            puntos_de_camino: vec![Coordenada {
                x: self.npc.x as i32,
                y: self.npc.y as i32,
            }],
            duracion: Some(between(20000.0, 120000.0)),
            npc_etiqueta: self.npc.etiqueta.clone(),
            silla_aleatoria: None,
        });
        self.contador = 0.0;
    }

    fn go_move(&mut self) {
        self.contador += 1.0;
        let destinacion = self.get_random_destination();
        self.caminos.push(Estado {
            estado: Movimiento::Move,
            puntos_de_camino: self.find_path(destinacion),
            npc_etiqueta: self.npc.etiqueta.clone(),
            duracion: None,
            silla_aleatoria: None,
        });

        self.npc.x = destinacion.x as f32;
        self.npc.y = destinacion.y as f32;
    }

    fn find_path(&self, destination: Coordenada) -> Vec<Coordenada> {
        let start: (i32, i32) = (self.npc.x.floor() as i32, self.npc.y.floor() as i32);
        let mut dest: Coordenada = destination;
        let mut attempts: i32 = 0;

        loop {
            let result = astar(
                &start,
                |p| self.mapa.vecinos(*p),
                |&(x, y)| ((x - dest.x as i32).abs() + (y - dest.y as i32).abs()) as u32,
                |&p| p == (dest.x as i32, dest.y as i32),
            );

            match result {
                Some((path, _cost)) => {
                    return path.into_iter().map(|(x, y)| Coordenada { x, y }).collect()
                }
                None => {
                    attempts += 1;
                    if attempts >= 5 {
                        println!(
                            "No se encontró camino después de varios intentos. {}",
                            self.npc.etiqueta
                        );
                        return Vec::new();
                    }

                    dest = self.get_random_destination();
                }
            }
        }
    }

    fn get_random_destination(&self) -> Coordenada {
        let mut x: i32;
        let mut y: i32;
        let mut attempts: u32 = 0;
        let max_attempts: u32 = 100;

        loop {
            x = rand::random::<i32>() % self.mundo.anchura as i32;
            y = rand::random::<i32>() % self.mundo.altura as i32;

            if x >= 0 && x < self.mundo.anchura as i32 && y >= 0 && y < self.mundo.altura as i32 {
                if !self.mapa.prohibidos[x as usize][y as usize] {
                    return Coordenada { x, y };
                }
            }

            attempts += 1;
            if attempts >= max_attempts {
                break;
            }
        }

        return Coordenada {
            x: self.npc.x.floor() as i32,
            y: self.npc.y.floor() as i32,
        };
    }

    fn go_sit(&mut self) {
        let sillas_disponibles = self.sillas.iter().filter(|silla| {
            !self
                .sillas_ocupadas
                .lock()
                .unwrap()
                .iter()
                .any(|silla_tomada: &Silla| silla_tomada.etiqueta == silla.etiqueta)
        });

        let silla_aleatoria = sillas_disponibles
            .clone()
            .nth(between(0.0, sillas_disponibles.count() as f32 - 1.0) as usize)
            .unwrap();

        self.sillas_ocupadas
            .lock()
            .unwrap()
            .push(silla_aleatoria.clone());

        let mut silla_x = silla_aleatoria.x_adjustado;
        let mut silla_y = silla_aleatoria.y_adjustado;

        let max_x = self.mapa.prohibidos.len();
        let max_y = if max_x > 0 {
            self.mapa.prohibidos[0].len()
        } else {
            0
        };

        if silla_x >= 0.0 && silla_x < max_x as f32 && silla_y >= 0.0 && silla_y < max_y as f32 {
            if self.mapa.prohibidos[silla_x as usize][silla_y as usize] {
                let nearest = self.find_nearest_walkable(silla_x as i32, silla_y as i32);
                silla_x = nearest.x as f32;
                silla_y = nearest.y as f32;
            }
        } else {
            println!("fail {}", self.npc.etiqueta);
        }

        self.silla_cerca = Some(Coordenada {
            x: silla_x as i32,
            y: silla_y as i32,
        });

        let bt: f32 = between(120000.0, 240000.0);

        self.caminos.push(Estado {
            estado: Movimiento::Sit,
            puntos_de_camino: self.find_path(self.silla_cerca.unwrap()),
            duracion: Some(bt),
            npc_etiqueta: self.npc.etiqueta.clone(),
            silla_aleatoria: Some(silla_aleatoria.etiqueta.clone()),
        });

        self.contador = 0.0;
        self.npc.x = self.silla_cerca.unwrap().x as f32;
        self.npc.y = self.silla_cerca.unwrap().y as f32;
        let sillas_ocupadas = Arc::clone(&self.sillas_ocupadas);
        let silla_aleatoria_etiqueta = silla_aleatoria.etiqueta.clone();
        self.reloj_juego.set_timeout(
            move || {
                let mut sillas_taken = sillas_ocupadas.lock().unwrap();
                sillas_taken.retain(|silla| silla.etiqueta != silla_aleatoria_etiqueta);
            },
            (bt / 600.0) as u64,
        );
    }

    fn find_nearest_walkable(&self, x: i32, y: i32) -> Coordenada {
        let mut current_y: i32 = y;

        while current_y < self.mundo.altura as i32 {
            if !self.mapa.prohibidos[x as usize][current_y as usize] {
                return Coordenada { x: x, y: current_y };
            }
            current_y += 1;
        }

        current_y = y;

        while current_y >= 0 {
            if !self.mapa.prohibidos[x as usize][current_y as usize] {
                return Coordenada { x, y: current_y };
            }
            current_y -= 1;
        }

        Coordenada { x, y }
    }

    fn clean_old_paths(&mut self) {
        if self.caminos.len() > 40 {
            self.caminos = self.caminos.split_off(self.caminos.len() - 40);
        }
    }
}
