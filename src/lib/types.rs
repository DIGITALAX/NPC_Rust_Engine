use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use warp::ws::Message;

#[derive(Debug, Serialize, Deserialize)]
pub struct Coordenada {
    pub x: f32,
    pub y: f32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Estado {
    pub estado: Movimiento,
    pub puntos_de_camino: Vec<Coordenada>,
    pub duracion: Option<f32>,
    pub npc_etiqueta: String,
    pub silla_aleatoria: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Articulo {
    pub uri: String,
    pub etiqueta: String,
    pub sitio: Coordenada,
    pub escala: Coordenada,
    pub talla: Coordenada,
    pub profundidad: Option<f32>,
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
pub struct Objeto {
    pub x: f32,
    pub y: f32,
    pub altura: f32,
    pub anchura: f32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Seat {
    pub x_adjustado: f32,
    pub y_adjustado: f32,
    pub profundidad: bool,
    pub depth: Option<f32>,
    pub anim: Direccion,
    pub etiqueta: String,
    pub sitio: Coordenada,
    pub talla: Coordenada,
    pub uri: String,
    pub escala: Coordenada,
    pub par: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
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
    pub escala: Coordenada,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum Movimiento {
    Move,
    Sit,
    Idle,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Fondo {
    etiqueta: String,
    uri: String,
    anchura: f32,
    altura: f32,
    sitio: Coordenada,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Prohibido {
    x: f32,
    y: f32,
    width: f32,
    height: f32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Escena {
    pub clave: String,
    pub mundo: Coordenada,
    pub imagen: String,
    pub prohibido: Vec<Prohibido>,
    pub profundidad: Vec<Articulo>,
    pub sillas: Vec<Seat>,
    pub fondo: Fondo,
    pub objetos: Vec<Articulo>,
    pub sprites: Vec<Sprite>,
}

pub type Clientes = Arc<Mutex<Vec<tokio::sync::mpsc::UnboundedSender<Message>>>>;
