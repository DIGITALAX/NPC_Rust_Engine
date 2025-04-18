use ethers::{
    abi::{Token, Tokenizable},
    contract::ContractInstance,
    core::k256::ecdsa::SigningKey,
    middleware::SignerMiddleware,
    providers::{Http, Provider},
    signers::Wallet,
    types::U256,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fmt,
    sync::{Arc, Mutex},
};
use strum::EnumIter;
use tokio::runtime::Handle;

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
pub struct Interactivo {
    pub uri: String,
    pub etiqueta: String,
    pub disenadores: Vec<String>,
    pub tipo: AutographType,
    pub sitio: Coordenada,
    pub escala: Escala,
    pub talla: Coordenada,
    pub profundidad: Option<f32>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum AutographType {
    NFT,
    Hoodie,
    Shirt,
    Catalog,
    Mix,
    All,
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
pub struct HalfSprite {
    pub id: u32,
    pub account_address: String,
    pub prompt: Prompt,
    pub billetera: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct EmptySprite {
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
    pub publicacion_reloj: u64,
    pub amigos: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Sprite {
    pub id: u32,
    pub etiqueta: String,
    pub uri: String,
    pub billetera: String,
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
    pub publicacion_reloj: u64,
    pub amigos: Vec<String>,
    pub account_address: String,
    pub prompt: Prompt,
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
    pub interactivos: Vec<Interactivo>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct EmptyEscena {
    pub clave: String,
    pub mundo: Talla,
    pub imagen: String,
    pub prohibido: Vec<Prohibido>,
    pub profundidad: Vec<Articulo>,
    pub sillas: Vec<Silla>,
    pub fondo: Fondo,
    pub objetos: Vec<Articulo>,
    pub sprites: Vec<EmptySprite>,
    pub interactivos: Vec<Interactivo>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SocketEscena {
    pub clave: String,
    pub mundo: Talla,
    pub imagen: String,
    pub prohibido: Vec<Prohibido>,
    pub profundidad: Vec<Articulo>,
    pub sillas: Vec<Silla>,
    pub fondo: Fondo,
    pub objetos: Vec<Articulo>,
    pub sprites: Vec<Sprite>,
    pub interactivos: Vec<Interactivo>,
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

#[allow(dead_code)]
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
pub struct Mention {
    pub content: String,
    pub id: String,
    pub post_id: String,
}

#[derive(Clone, Debug)]
pub struct EscenaEstudio {
    pub clave: String,
    pub sillas_ocupadas: Arc<Mutex<Vec<Silla>>>,
    pub npcs: Vec<NPCAleatorio>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Text {
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MessageExample {
    pub user: String,
    pub content: Text,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Prompt {
    pub bio: String,
    pub lore: String,
    pub style: String,
    pub knowledge: String,
    pub adjectives: String,
    pub model: String,
    pub custom_instructions: String,
    pub message_examples: Vec<Vec<MessageExample>>,
    pub cover: String,
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
    pub reloj_au: u64,
    pub silla_cerca: Option<Coordenada>,
    pub mapa: Mapa,
    pub escena: String,
    pub ultimo_tiempo_comprobacion: u64,
    pub spectator_rewards_contrato: Arc<
    ContractInstance<
        Arc<SignerMiddleware<Arc<Provider<Http>>, Wallet<SigningKey>>>,
        SignerMiddleware<Arc<Provider<Http>>, Wallet<SigningKey>>,
    >,
>,
    pub autograph_catalog_contrato: Arc<
        ContractInstance<
            Arc<SignerMiddleware<Arc<Provider<Http>>, Wallet<SigningKey>>>,
            SignerMiddleware<Arc<Provider<Http>>, Wallet<SigningKey>>,
        >,
    >,
    pub tokens: Option<TokensAlmacenados>,
    pub registro_tipos: Vec<LensType>,
    pub registro_paginas: Vec<U256>,
    pub registro_colecciones: Vec<U256>,
    pub ultima_mencion: String,
    pub manija: Handle,
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

impl fmt::Debug for NPCAleatorio {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("NPCAleatorio")
            .field("escena", &self.escena)
            .field("npc", &self.npc)
            .field("contador", &self.contador)
            .field("movimientos_max", &self.movimientos_max)
            .field("silla_cerca", &self.silla_cerca)
            .field(
                "ultimo_tiempo_comprobacion",
                &self.ultimo_tiempo_comprobacion,
            )
            .field("tokens", &self.tokens)
            .field("registro_tipos", &self.registro_tipos)
            .field("registro_paginas", &self.registro_paginas)
            .field("registro_colecciones", &self.registro_colecciones)
            .field("ultima_mencion", &self.ultima_mencion)
            .finish()
    }
}

#[derive(Clone)]
pub struct Mapa {
    pub anchura: usize,
    pub altura: usize,
    pub prohibidos: Vec<Vec<bool>>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct IpfsRespuesta {
    Name: String,
    pub Hash: String,
    Size: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Contenido {
    pub mainContentFocus: String,
    pub title: String,
    pub content: String,
    pub id: String,
    pub locale: String,
    pub tags: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image: Option<Imagen>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Publicacion {
    #[serde(rename = "$schema")]
    pub schema: String,
    pub lens: Contenido,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct Imagen {
    #[serde(rename = "type")]
    pub tipo: String,
    pub item: String,
}

#[derive(Copy, PartialEq, EnumIter, Clone, Deserialize, Debug)]
pub enum LensType {
    Catalog,
    Comment,
    Publication,
    Autograph,
    Quote,
    Mirror,
}

impl TryFrom<u8> for LensType {
    type Error = ();

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(LensType::Catalog),
            1 => Ok(LensType::Comment),
            2 => Ok(LensType::Publication),
            3 => Ok(LensType::Autograph),
            4 => Ok(LensType::Quote),
            5 => Ok(LensType::Mirror),
            _ => Err(()),
        }
    }
}

impl Tokenizable for LensType {
    fn from_token(token: Token) -> Result<Self, ethers::abi::InvalidOutputType> {
        match token {
            Token::Uint(val) if val == 0u64.into() => Ok(LensType::Catalog),
            Token::Uint(val) if val == 1u64.into() => Ok(LensType::Comment),
            Token::Uint(val) if val == 2u64.into() => Ok(LensType::Publication),
            Token::Uint(val) if val == 3u64.into() => Ok(LensType::Autograph),
            Token::Uint(val) if val == 4u64.into() => Ok(LensType::Quote),
            Token::Uint(val) if val == 5u64.into() => Ok(LensType::Mirror),
            _ => Err(ethers::abi::InvalidOutputType(
                "Unexpected token".to_string(),
            )),
        }
    }

    fn into_token(self) -> Token {
        match self {
            LensType::Catalog => Token::Uint(0u64.into()),
            LensType::Comment => Token::Uint(1u64.into()),
            LensType::Publication => Token::Uint(2u64.into()),
            LensType::Autograph => Token::Uint(3u64.into()),
            LensType::Quote => Token::Uint(4u64.into()),
            LensType::Mirror => Token::Uint(5u64.into()),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct LensTokens {
    pub access_token: String,
    pub refresh_token: String,
    pub id_token: String,
}

#[derive(Debug, Clone)]
pub struct TokensAlmacenados {
    pub tokens: LensTokens,
    pub expiry: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Coleccion {
    pub imagen: String,
    pub descripcion: String,
    pub coleccion_id: String,
}
