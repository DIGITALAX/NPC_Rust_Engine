use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use warp::ws::Message;

#[derive(Debug, Serialize, Deserialize)]
pub struct Coordenada {
    pub x: i32,
    pub y: i32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Escala {
    pub x: f32,
    pub y: f32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Talla {
    pub anchura: i32,
    pub altura: i32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Estado {
    pub estado: Movimiento,
    pub puntos_de_camino: Vec<Coordenada>,
    pub duracion: Option<i32>,
    pub npc_etiqueta: String,
    pub silla_aleatoria: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Articulo {
    pub uri: String,
    pub etiqueta: String,
    pub sitio: Coordenada,
    pub escala: Escala,
    pub talla: Coordenada,
    pub profundidad: Option<i32>,
}

#[derive(Debug, Serialize, Deserialize)]
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

#[derive(Debug, Serialize, Deserialize)]
pub struct Silla {
    pub x_adjustado: i32,
    pub y_adjustado: i32,
    pub profundidad: bool,
    pub depth: Option<i32>,
    pub anim: Direccion,
    pub etiqueta: String,
    pub sitio: Coordenada,
    pub talla: Coordenada,
    pub uri: String,
    pub escala: Escala,
    pub par: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Sprite {
    pub etiqueta: String,
    pub uri: String,
    pub x: i32,
    pub y: i32,
    pub altura: i32,
    pub anchura: i32,
    pub anchura_borde: i32,
    pub altura_borde: i32,
    pub margen: i32,
    pub tapa: String,
    pub marco_inicio: i32,
    pub marco_final: i32,
    pub movimientos_max: i32,
    pub escala: Escala,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum Movimiento {
    Move,
    Sit,
    Idle,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Fondo {
    pub etiqueta: String,
    pub uri: String,
    pub anchura: i32,
    pub altura: i32,
    pub sitio: Coordenada,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Prohibido {
    pub x: i32,
    pub y: i32,
    pub anchura: i32,
    pub altura: i32,
}

#[derive(Debug, Serialize, Deserialize)]
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

pub type Clientes = Arc<Mutex<Vec<tokio::sync::mpsc::UnboundedSender<Message>>>>;
