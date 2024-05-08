use crate::lib::{
    types::{Coordenada, Estado, GameTimer, Movimiento, NPCAleatorio, Silla, Sprite, Talla},
    utils::between,
};
use pathfinding::prelude::astar;
use std::sync::{Arc, Mutex};

impl NPCAleatorio {
    pub fn new(
        sprite: Sprite,
        sillas_ocupadas: Arc<Mutex<Vec<Silla>>>,
        sillas: Vec<Silla>,
        mundo: Talla,
    ) -> Self {
        NPCAleatorio {
            reloj_juego: GameTimer::new(),
            sillas_ocupadas,
            sillas,
            mundo,
            movimientos_max: sprite.movimientos_max,
            caminos: Vec::new(),
            npc: sprite,
            contador: 0.0,
            silla_cerca: None,
        }
    }

    pub fn get_state(&self) -> Vec<Estado> {
        self.caminos.clone()
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
        let current_npc = Coordenada {
            x: self.npc.x as i32,
            y: self.npc.y as i32,
        };

        let path = astar(
            &current_npc,
            |p| self.successors(*p),
            |p| distance(p, &destination) as u32,
            |p| *p == destination,
        );

        path.map(|(path, _)| path).unwrap_or_else(Vec::new)
    }

    fn successors(&self, p: Coordenada) -> Vec<(Coordenada, u32)> {
        let directions = vec![(1.0, 0.0), (-1.0, 0.0), (0.0, 1.0), (0.0, -1.0)];

        directions
            .into_iter()
            .filter_map(|(dx, dy)| {
                let new_point = Coordenada {
                    x: p.x + dx as i32,
                    y: p.y + dy as i32,
                };

                if self.is_walkable(new_point) {
                    Some((new_point, 1))
                } else {
                    None
                }
            })
            .collect()
    }

    fn is_walkable(&self, p: Coordenada) -> bool {
        p.x >= 0 && p.x < self.mundo.anchura as i32 && p.y >= 0 && p.y < self.mundo.altura as i32
    }

    fn get_random_destination(&self) -> Coordenada {
        let mut x: i32;
        let mut y: i32;
        let mut attempts: f32 = 0.0;
        let min_distance: f32 = 500.0;

        loop {
            x = rand::random::<i32>() * self.mundo.anchura as i32;
            y = rand::random::<i32>() * self.mundo.altura as i32;
            attempts += 1.0;

            if attempts > 100.0 {
                break;
            }

            if self.is_walkable(Coordenada { x, y })
                && distance(
                    &Coordenada { x, y },
                    &Coordenada {
                        x: self.npc.x as i32,
                        y: self.npc.y as i32,
                    },
                ) >= min_distance
            {
                break;
            }
        }

        Coordenada { x, y }
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

        if !self.is_walkable(Coordenada {
            x: silla_x as i32,
            y: silla_y as i32,
        }) {
            let nearest = self.find_nearest_walkable(silla_x as i32, silla_y as i32);
            silla_x = nearest.x as f32;
            silla_y = nearest.y as f32;
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

        let sillas_taken = Arc::clone(&self.sillas_ocupadas);

        self.reloj_juego.set_timeout(
            move || {
                let mut sillas_taken = sillas_taken.lock().unwrap();
                sillas_taken.retain(|silla| silla.etiqueta != silla_aleatoria.etiqueta);
            },
            (bt / 600.0) as u64,
        );
    }

    fn find_nearest_walkable(&self, x: i32, y: i32) -> Coordenada {
        let mut current_y: i32 = y;

        while current_y < self.mundo.altura as i32 {
            if self.is_walkable(Coordenada { x, y: current_y }) {
                return Coordenada { x, y: current_y };
            }
            current_y += 1;
        }

        current_y = y;

        while current_y >= 0 {
            if self.is_walkable(Coordenada {
                x,
                y: current_y as i32,
            }) {
                return Coordenada {
                    x,
                    y: current_y as i32,
                };
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

fn distance(a: &Coordenada, b: &Coordenada) -> f32 {
    ((a.x - b.x).abs() + (a.y - b.y).abs()) as f32
}
