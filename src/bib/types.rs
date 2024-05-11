use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct Coordenada {
    pub x: i32,
    pub y: i32,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Escala {
    pub x: f32,
    pub y: f32,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Talla {
    pub anchura: f32,
    pub altura: f32,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Estado {
    pub estado: Movimiento,
    pub puntos_de_camino: Vec<Coordenada>,
    pub duracion: Option<f32>,
    pub npc_etiqueta: String,
    pub silla_aleatoria: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Articulo {
    pub uri: String,
    pub etiqueta: String,
    pub sitio: Coordenada,
    pub escala: Escala,
    pub talla: Coordenada,
    pub profundidad: Option<f32>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum Direccion {
    #[serde(rename = "izquierda")]
    Izquierda,
    #[serde(rename = "derecha")]
    Derecha,
    #[serde(rename = "arriba")]
    Arriba,
    #[serde(rename = "abajo")]
    Abajo,
    #[serde(rename = "izquierdaArriba")]
    IzquierdaArriba,
    #[serde(rename = "izquierdaAbajo")]
    IzquierdaAbajo,
    #[serde(rename = "derechaArriba")]
    DerechaArriba,
    #[serde(rename = "derechaAbajo")]
    DerechaAbajo,
    #[serde(rename = "inactivo")]
    Inactivo,
    #[serde(rename = "sentadoSofa")]
    Sofa,
    #[serde(rename = "sentadoEscritorio")]
    Silla,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Silla {
    pub x_adjustado: f32,
    pub y_adjustado: f32,
    pub profundidad: bool,
    pub depth: Option<f32>,
    pub anim: Direccion,
    pub etiqueta: String,
    pub sitio: Coordenada,
    pub talla: Coordenada,
    pub uri: String,
    pub escala: Escala,
    pub par: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Sprite {
    pub etiqueta: String,
    pub uri: String,
    pub x: f32,
    pub y: f32,
    pub altura: f32,
    pub anchura: f32,
    pub anchura_borde: f32,
    pub altura_borde: f32,
    pub margen: f32,
    pub tapa: String,
    pub marco_inicio: f32,
    pub marco_final: f32,
    pub movimientos_max: f32,
    pub escala: Escala,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum Movimiento {
    Move,
    Sit,
    Idle,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Fondo {
    pub etiqueta: String,
    pub uri: String,
    pub anchura: f32,
    pub altura: f32,
    pub sitio: Coordenada,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Prohibido {
    pub x: f32,
    pub y: f32,
    pub anchura: f32,
    pub altura: f32,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Escena {
    pub clave: String,
    pub mundo: Talla,
    pub imagen: String,
    pub prohibido: Vec<Prohibido>,
    pub profundidad: Vec<Articulo>,
    pub sillas: Vec<Silla>,
    pub fondo: Fondo,
    pub objetos: Vec<Articulo>,
    pub sprites: Vec<Sprite>,
}

#[derive(Clone)]
pub struct GameTimer {
    pub ticks: u32,
    pub time_accumulated: u64,
    pub tasks: Vec<Task>,
}

#[derive(Clone)]
pub struct Task {
    pub execute_on_ms: u64,
    pub callback: CloneableCallback,
}

pub struct CloneableCallback {
    callback: Arc<Box<dyn Fn() + Send + Sync + 'static>>,
}

impl CloneableCallback {
    pub fn new<F>(callback: F) -> Self
    where
        F: Fn() + 'static + Send + Sync,
    {
        Self {
            callback: Arc::new(Box::new(callback)),
        }
    }
}

impl std::ops::Deref for CloneableCallback {
    type Target = dyn Fn();

    fn deref(&self) -> &Self::Target {
        &*self.callback
    }
}
impl Clone for CloneableCallback {
    fn clone(&self) -> Self {
        let callback_clone = self.callback.clone();
        Self {
            callback: callback_clone,
        }
    }
}

impl std::fmt::Debug for CloneableCallback {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.write_str("CloneableCallback")
    }
}

#[derive(Debug, Serialize)]
pub enum RespuestaTrabajadora<'a> {
    StateResponse {
        cmd: String,
        clave: String,
        estados: Vec<&'a Vec<Estado>>,
    },
    Error {
        mensaje: String,
    },
}

#[derive(Clone, Debug)]
pub struct EscenaEstudio {
    pub clave: String,
    pub sillas_ocupadas: Vec<Silla>,
    pub npcs: Vec<NPCAleatorio>,
}

#[derive(Clone)]
pub struct NPCAleatorio {
    pub sillas: Vec<Silla>,
    pub mundo: Talla,
    pub movimientos_max: f32,
    pub caminos: Vec<Estado>,
    pub npc: Sprite,
    pub sillas_ocupadas: Arc<Mutex<Vec<Silla>>>,
    pub contador: f32,
    pub reloj_juego: GameTimer,
    pub silla_cerca: Option<Coordenada>,
    pub mapa: Mapa,
}

#[derive(Clone)]
pub struct NPCStudioEngine {
    pub escenas: Arc<Mutex<HashMap<String, EscenaEstudio>>>,
}

pub struct EngineWrapper {
    pub engine: Arc<NPCStudioEngine>,
}

impl Clone for EngineWrapper {
    fn clone(&self) -> Self {
        Self {
            engine: Arc::clone(&self.engine),
        }
    }
}

impl std::fmt::Debug for NPCAleatorio {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "NPCAleatorio {{ /* campos personalizados aquí */ }}")
    }
}

#[derive(Clone)]
pub struct Mapa {
    pub anchura: usize,
    pub altura: usize,
    pub prohibidos: Vec<Vec<bool>>,
}
