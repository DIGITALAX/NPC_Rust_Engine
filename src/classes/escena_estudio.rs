use crate::bib::types::{
    Escena, EscenaEstudio, Estado, GameTimer, Mapa, NPCAleatorio, Prohibido, RespuestaTrabajadora,
    Talla,
};
use rayon::prelude::*;
use std::sync::{Arc, Mutex};
use tokio::runtime::Handle;

impl EscenaEstudio {
    pub fn new(escena: Escena, manija: Handle) -> Self {
        let sprites = escena.sprites.clone();
        let prohibidos = escena.prohibido.clone();
        let anchura = escena.mundo.anchura - ((300.0 * 0.5) * 0.5);
        let altura = escena.mundo.altura - ((600.0 * 0.5) * 0.5);

        let mapa = Mapa::new(anchura as usize, altura as usize, prohibidos);
        let sillas_ocupadas = Arc::new(Mutex::new(Vec::new()));

        let npcs: Vec<NPCAleatorio> = sprites
            .iter()
            .map(|sprite| {
                let npc = NPCAleatorio::new(
                    sprite.clone(),
                    Arc::clone(&sillas_ocupadas),
                    escena.sillas.clone(),
                    Talla { anchura, altura },
                    GameTimer::new(),
                    mapa.clone(),
                    escena.clave.to_string(),
                    manija.clone(),
                );
                npc
            })
            .collect();

        EscenaEstudio {
            clave: escena.clave,
            sillas_ocupadas,
            npcs,
        }
    }

    pub fn ejecutar_bucle(&mut self, delta: u64) {
        self.npcs.par_iter_mut().for_each(|npc| {
            npc.actualizar(delta);
        });
    }

    pub fn request_state(&mut self) -> Option<RespuestaTrabajadora> {
        let mut estados: Vec<&Vec<Estado>> = Vec::new();

        for npc in &mut self.npcs {
            estados.push(npc.conseguir_estado());
        }

        if estados.is_empty() {
            None
        } else {
            Some(RespuestaTrabajadora::StateResponse {
                cmd: "stateResponse".to_string(),
                clave: self.clave.clone(),
                estados,
            })
        }
    }
}

impl Mapa {
    pub fn new(anchura: usize, altura: usize, prohibidos: Vec<Prohibido>) -> Mapa {
        let mut blocked = vec![vec![false; altura]; anchura];

        for prohibido in prohibidos {
            let start_x = prohibido.x as usize;
            let start_y = prohibido.y as usize;
            let end_x = (prohibido.x + prohibido.anchura) as usize;
            let end_y = (prohibido.y + prohibido.altura) as usize;

            for x in start_x..end_x {
                for y in start_y..end_y {
                    if x < anchura && y < altura {
                        blocked[x][y] = true;
                    }
                }
            }
        }

        Mapa {
            anchura,
            altura,
            prohibidos: blocked,
        }
    }

    pub fn vecinos(&self, (x, y): (i32, i32)) -> Vec<((i32, i32), u32)> {
        let mut vecinos = Vec::new();
        for dx in -1..=1 {
            for dy in -1..=1 {
                if dx == 0 && dy == 0 {
                    continue;
                }
                let nx = x + dx;
                let ny = y + dy;
                if nx >= 0
                    && nx < self.anchura as i32
                    && ny >= 0
                    && ny < self.altura as i32
                    && !self.prohibidos[nx as usize][ny as usize]
                {
                    vecinos.push(((nx, ny), if dx != 0 && dy != 0 { 14 } else { 10 }));
                }
            }
        }
        vecinos
    }
}
