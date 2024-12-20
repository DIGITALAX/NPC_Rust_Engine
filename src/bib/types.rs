use chrono::{DateTime, Utc};
use core::fmt;
use ethers::{
    abi::{Token, Tokenizable, Tokenize},
    contract::ContractInstance,
    core::k256::ecdsa::SigningKey,
    middleware::SignerMiddleware,
    providers::{Http, Provider},
    signers::Wallet,
    types::{Address, Bytes, U256},
};
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    error::Error,
    sync::{Arc, Mutex},
};
use tokio::{runtime::Handle, sync::RwLock};

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

#[derive(Debug, Clone)]
pub struct Prompt {
    pub personalidad: String,
    pub idiomas: Vec<String>,
    pub temas: Arc<Mutex<HashMap<String, Vec<String>>>>,
    pub tono: Arc<Mutex<HashMap<String, Vec<String>>>>,
    pub imagenes: Arc<Mutex<Vec<String>>>,
    pub amigos: Vec<U256>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct PromptHelper {
    personalidad: String,
    idiomas: Vec<String>,
    imagenes: Vec<String>,
    amigos: Vec<U256>,
    temas: HashMap<String, Vec<String>>,
    tono: HashMap<String, Vec<String>>,
}

impl Serialize for Prompt {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let temas = self.temas.lock().unwrap().clone();
        let tono = self.tono.lock().unwrap().clone();

        let helper = PromptHelper {
            personalidad: self.personalidad.clone(),
            idiomas: self.idiomas.clone(),
            imagenes: self.imagenes.lock().unwrap().clone(),
            amigos: self.amigos.clone(),
            tono,
            temas,
        };
        helper.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Prompt {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let helper = PromptHelper::deserialize(deserializer)?;
        let temas = Arc::new(Mutex::new(helper.temas.clone()));
        let tono = Arc::new(Mutex::new(helper.tono.clone()));
        Ok(Prompt {
            personalidad: helper.personalidad,
            idiomas: helper.idiomas,
            imagenes: Arc::new(Mutex::new(helper.imagenes)),
            amigos: helper.amigos,
            tono,
            temas,
        })
    }
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
pub struct Sprite {
    pub etiqueta: String,
    pub uri: String,
    pub billetera: String,
    pub x: f32,
    pub y: f32,
    pub perfil_id: U256,
    pub altura: f32,
    pub anchura: f32,
    pub anchura_borde: f32,
    pub altura_borde: f32,
    pub margen: f32,
    pub tapa: String,
    pub tapa_dos: String,
    pub marco_inicio: f32,
    pub marco_final: f32,
    pub movimientos_max: f32,
    pub escala: Escala,
    pub publicacion_reloj: u64,
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
pub struct EscenaEstudio {
    pub clave: String,
    pub sillas_ocupadas: Arc<Mutex<Vec<Silla>>>,
    pub npcs: Vec<NPCAleatorio>,
}

#[derive(Clone)]
pub struct NPCAleatorio {
    pub sillas: Vec<Silla>,
    pub reloj_semanal: chrono::DateTime<Utc>,
    pub pesos_manejados: Arc<Mutex<bool>>,
    pub alquiler_pagado: Arc<Mutex<i32>>,
    pub mundo: Talla,
    pub movimientos_max: f32,
    pub caminos: Vec<Estado>,
    pub npc: Sprite,
    pub sillas_ocupadas: Arc<Mutex<Vec<Silla>>>,
    pub contador: f32,
    pub reloj_juego: GameTimer,
    pub silla_cerca: Option<Coordenada>,
    pub mapa: Mapa,
    pub escena: String,
    pub ultimo_tiempo_comprobacion: u64,
    pub ultimo_tiempo_mencion: u64,
    pub lens_hub_contrato: Arc<
        ContractInstance<
            Arc<SignerMiddleware<Arc<Provider<Http>>, Wallet<SigningKey>>>,
            SignerMiddleware<Arc<Provider<Http>>, Wallet<SigningKey>>,
        >,
    >,
    pub autograph_data_contrato: Arc<
        ContractInstance<
            Arc<SignerMiddleware<Arc<Provider<Http>>, Wallet<SigningKey>>>,
            SignerMiddleware<Arc<Provider<Http>>, Wallet<SigningKey>>,
        >,
    >,
    pub npc_publication_contrato: Arc<
        ContractInstance<
            Arc<SignerMiddleware<Arc<Provider<Http>>, Wallet<SigningKey>>>,
            SignerMiddleware<Arc<Provider<Http>>, Wallet<SigningKey>>,
        >,
    >,
    pub npc_rent_contrato: Arc<
        ContractInstance<
            Arc<SignerMiddleware<Arc<Provider<Http>>, Wallet<SigningKey>>>,
            SignerMiddleware<Arc<Provider<Http>>, Wallet<SigningKey>>,
        >,
    >,
    pub npc_access_contrato: Arc<
        ContractInstance<
            Arc<SignerMiddleware<Arc<Provider<Http>>, Wallet<SigningKey>>>,
            SignerMiddleware<Arc<Provider<Http>>, Wallet<SigningKey>>,
        >,
    >,
    pub manija: Handle,
    pub tokens: Option<TokensAlmacenados>,
    pub estado: Arc<RwLock<EstadoNPC>>,
    pub ultima_mencion_procesada: Arc<RwLock<DateTime<Utc>>>,
    pub menciones_procesadas: Arc<RwLock<HashSet<String>>>,
    pub boudica: bool,
    pub llama_recibido: Option<String>,
}

#[derive(Clone, Copy, PartialEq)]
pub enum EstadoNPC {
    Activo,
    Inactivo,
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

#[derive(Serialize, Deserialize, Debug)]
pub struct Pub {
    pub profileId: U256,
    pub contentURI: String,
    pub actionModules: Vec<Address>,
    pub actionModulesInitDatas: Vec<Bytes>,
    pub referenceModule: Address,
    pub referenceModuleInitData: Bytes,
}

impl Tokenize for Pub {
    fn into_tokens(self) -> Vec<Token> {
        vec![
            Token::Uint(self.profileId),
            Token::String(self.contentURI),
            Token::Array(
                self.actionModules
                    .into_iter()
                    .map(|addr| addr.into_token())
                    .collect(),
            ),
            Token::Array(
                self.actionModulesInitDatas
                    .into_iter()
                    .map(|data| Token::Bytes(data.to_vec()))
                    .collect(),
            ),
            self.referenceModule.into_token(),
            Token::Bytes(self.referenceModuleInitData.to_vec()),
        ]
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Mirror {
    pub profileId: U256,
    pub metadataURI: String,
    pub pointedProfileId: U256,
    pub pointedPubId: U256,
    pub referrerProfileIds: Vec<U256>,
    pub referrerPubIds: Vec<U256>,
    pub referenceModuleData: Bytes,
}

impl Tokenize for Mirror {
    fn into_tokens(self) -> Vec<Token> {
        vec![
            Token::Uint(self.profileId),
            Token::String(self.metadataURI),
            Token::Uint(self.pointedProfileId),
            Token::Uint(self.pointedPubId),
            Token::Array(
                self.referrerProfileIds
                    .into_iter()
                    .map(|uint| uint.into_token())
                    .collect(),
            ),
            Token::Array(
                self.referrerPubIds
                    .into_iter()
                    .map(|uint| uint.into_token())
                    .collect(),
            ),
            Token::Bytes(self.referenceModuleData.to_vec()),
        ]
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Comment {
    pub profileId: U256,
    pub contentURI: String,
    pub pointedProfileId: U256,
    pub pointedPubId: U256,
    pub referrerProfileIds: Vec<U256>,
    pub referrerPubIds: Vec<U256>,
    pub referenceModuleData: Bytes,
    pub actionModules: Vec<Address>,
    pub actionModulesInitDatas: Vec<Bytes>,
    pub referenceModule: Address,
    pub referenceModuleInitData: Bytes,
}

impl Tokenize for Comment {
    fn into_tokens(self) -> Vec<Token> {
        vec![
            Token::Uint(self.profileId),
            Token::String(self.contentURI),
            Token::Uint(self.pointedProfileId),
            Token::Uint(self.pointedPubId),
            Token::Array(
                self.referrerProfileIds
                    .into_iter()
                    .map(|uint| uint.into_token())
                    .collect(),
            ),
            Token::Array(
                self.referrerPubIds
                    .into_iter()
                    .map(|uint| uint.into_token())
                    .collect(),
            ),
            Token::Bytes(self.referenceModuleData.to_vec()),
            Token::Array(
                self.actionModules
                    .into_iter()
                    .map(|addr| addr.into_token())
                    .collect(),
            ),
            Token::Array(
                self.actionModulesInitDatas
                    .into_iter()
                    .map(|data| Token::Bytes(data.to_vec()))
                    .collect(),
            ),
            self.referenceModule.into_token(),
            Token::Bytes(self.referenceModuleInitData.to_vec()),
        ]
    }
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
    pub appId: String,
    pub id: String,
    pub hideFromFeed: bool,
    pub locale: String,
    pub tags: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image: Option<Imagen>,
    pub attributes: Option<Vec<MetadataAttribute>>,
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

#[derive(Clone)]
pub struct Llama;

#[derive(PartialEq, Clone, Deserialize, Debug)]
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

impl LensType {
    pub fn as_u8(&self) -> u8 {
        match self {
            LensType::Catalog => 0,
            LensType::Comment => 1,
            LensType::Publication => 2,
            LensType::Autograph => 3,
            LensType::Quote => 4,
            LensType::Mirror => 5,
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

pub struct Boudica {
    pub _language: String,
    pub _pageNumber: u8,
}

impl Tokenize for Boudica {
    fn into_tokens(self) -> Vec<Token> {
        vec![
            Token::String(self._language).into_token(),
            Token::Uint(U256::from(self._pageNumber)).into_token(),
        ]
    }
}

pub struct PublicacionPrediccion {
    pub _locale: String,
    pub _npcWallet: Address,
    pub _boudica: bool,
}

impl Tokenize for PublicacionPrediccion {
    fn into_tokens(self) -> Vec<Token> {
        vec![
            Token::String(self._locale).into_token(),
            Token::Address(self._npcWallet).into_token(),
            Token::Bool(self._boudica).into_token(),
        ]
    }
}

#[derive(Debug)]
pub struct RegisterPub {
    pub _tensors: String,
    pub _locale: String,
    pub _collection: U256,
    pub _profileId: U256,
    pub _pubId: U256,
    pub _pageNumber: u8,
    pub _lensType: u8,
    pub _boudica: bool,
}

// impl Tokenize for RegisterPub {
//     fn into_tokens(self) -> Vec<Token> {
//         vec![
//             Token::String(self._tensors).into_token(),
//             Token::String(self._locale).into_token(),
//             Token::Uint(self._collection).into_token(),
//             Token::Uint(self._profileId).into_token(),
//             Token::Uint(self._pubId).into_token(),
//             Token::Uint(U256::from(self._pageNumber)).into_token(),
//             Token::Bool(self._boudica).into_token(),
//             self._lensType.into_token(),
//         ]
//     }
// }

impl Tokenize for RegisterPub {
    fn into_tokens(self) -> Vec<Token> {
        vec![
            Token::String(self._tensors),
            Token::String(self._locale),
            Token::Uint(self._collection),
            Token::Uint(self._profileId),
            Token::Uint(self._pubId),
            Token::Uint(U256::from(self._pageNumber)),
            Token::Uint(U256::from(self._lensType)),
            Token::Bool(self._boudica),
        ]
    }
}

#[derive(Debug)]
pub struct CustomError {
    details: String,
}

impl CustomError {
    pub fn new(msg: &str) -> CustomError {
        CustomError {
            details: msg.to_string(),
        }
    }
}

impl fmt::Display for CustomError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.details)
    }
}

impl Error for CustomError {}
unsafe impl Send for CustomError {}
unsafe impl Sync for CustomError {}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct LensTokens {
    pub access_token: String,
    pub refresh_token: String,
    pub identity_token: String,
}

#[derive(Debug, Clone)]
pub struct TokensAlmacenados {
    pub tokens: LensTokens,
    pub expira_en: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct LlamaOpciones {
    pub num_keep: i32,
    pub seed: i32,
    pub num_predict: i32,
    pub top_k: i32,
    pub top_p: f32,
    pub min_p: f32,
    pub ctx: i32,
    pub tfs_z: f32,
    pub typical_p: f32,
    pub repeat_last_n: i32,
    pub temperature: f32,
    pub repeat_penalty: f32,
    pub presence_penalty: f32,
    pub frequency_penalty: f32,
    pub mirostat: i32,
    pub mirostat_tau: f32,
    pub mirostat_eta: f32,
    pub penalize_newline: bool,
    pub numa: bool,
    pub num_tokens: i32,
    pub num_batch: i32,
    pub num_gpu: i32,
    pub main_gpu: i32,
    pub low_vram: bool,
    pub f16_kv: bool,
    pub vocab_only: bool,
    pub use_mmap: bool,
    pub use_mlock: bool,
    pub num_thread: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlamaRespuesta {
    pub response: String,
    pub json: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetadataAttribute {
    pub key: String,
    #[serde(rename = "type")]
    pub tipo: String,
    pub value: String,
}
