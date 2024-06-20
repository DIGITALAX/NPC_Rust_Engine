use crate::bib::utils::subir_ipfs_imagen;
use crate::bib::{lens, utils::subir_ipfs};
use crate::{
    bib::{
        types::{
            Contenido, Coordenada, CustomError, Estado, GameTimer, Imagen, LensType, Movimiento,
            NPCAleatorio, Publicacion, RegisterPub, Silla, Sprite, Talla,
        },
        utils::between,
    },
    Mapa, Pub,
};
use crate::{Llama, LENS_HUB_PROXY, NPC_PUBLICATION};
use abi::{Token, Tokenize};
use ethers::prelude::*;
use ethers::types::{Address, Bytes, U256};
use pathfinding::prelude::astar;
use rand::prelude::SliceRandom;
use rand::SeedableRng;
use rand_chacha::ChaCha12Rng;
use serde_json::to_string;
use std::{
    error::Error,
    sync::{Arc, Mutex},
};
use tokio::sync::Mutex as TokioMutex;
use uuid::Uuid;

impl NPCAleatorio {
    pub fn new(
        sprite: Sprite,
        sillas_ocupadas: Arc<Mutex<Vec<Silla>>>,
        sillas: Vec<Silla>,
        mundo: Talla,
        reloj_juego: GameTimer,
        mapa: Mapa,
        escena: String,
    ) -> Self {
        let (lens_hub_contrato, autograph_data_contrato, npc_publication_contrato) =
            lens::inicializar_contrato(&sprite.etiqueta.to_string());

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
            ultimo_tiempo_comprobacion: 0,
            lens_hub_contrato,
            autograph_data_contrato,
            npc_publication_contrato,
            escena,
        }
    }

    pub fn conseguir_estado(&self) -> &Vec<Estado> {
        &self.caminos
    }

    pub fn actualizar(&mut self, delta_time: u64) {
        self.reloj_juego.tick(delta_time);
        self.elegir_direccion_aleatoria();
        self.limpiar_caminos();

        if self.ultimo_tiempo_comprobacion > 0 {
            self.ultimo_tiempo_comprobacion -= delta_time;
        }

        if self.ultimo_tiempo_comprobacion <= 0 {
            self.ultimo_tiempo_comprobacion = 1000000;
            println!("dentro");
            self.comprobar_conversacion();
        }
    }

    fn elegir_direccion_aleatoria(&mut self) {
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
                self.sentar();
            } else {
                self.inactivo();
            }
        } else {
            self.mover();
        }
    }

    fn inactivo(&mut self) {
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

    fn mover(&mut self) {
        self.contador += 1.0;
        let destinacion = self.obtener_destinacion_aleatoria();
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
                    if attempts >= 10 {
                        println!(
                            "No se encontró camino después de varios intentos. {}",
                            self.npc.etiqueta
                        );
                        return Vec::new();
                    }

                    dest = self.obtener_destinacion_aleatoria();
                }
            }
        }
    }

    fn obtener_destinacion_aleatoria(&self) -> Coordenada {
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

    fn sentar(&mut self) {
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

        let mut nearest = Coordenada { x: 0, y: 0 };

        if silla_x >= self.mundo.anchura {
            nearest = self.encontrar_camino_cercano(self.mundo.anchura as i32, silla_y as i32);
        } else if silla_x < 0.0 {
            nearest = self.encontrar_camino_cercano(1, silla_y as i32);
        } else if silla_y >= self.mundo.altura {
            nearest = self.encontrar_camino_cercano(silla_x as i32, self.mundo.altura as i32 - 1);
        } else if silla_y < 0.0 {
            nearest = self.encontrar_camino_cercano(silla_x as i32, 1);
        } else if self.mapa.prohibidos[silla_x as usize][silla_y as usize] {
            nearest = self.encontrar_camino_cercano(silla_x as i32, silla_y as i32 - 1);
        }

        if nearest.x > 0 && nearest.y > 0 {
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

    fn encontrar_camino_cercano(&self, x: i32, y: i32) -> Coordenada {
        let mut current_y: i32 = y;

        while current_y < self.mundo.altura as i32 {
            if !self.mapa.prohibidos[x as usize][current_y as usize] {
                return Coordenada { x, y: current_y };
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

    fn limpiar_caminos(&mut self) {
        if self.caminos.len() > 40 {
            self.caminos = self.caminos.split_off(self.caminos.len() - 40);
        }
    }

    fn comprobar_conversacion(&mut self) {
        let llama = Llama;
        let npc_clone = Arc::new(self.clone());

        tokio::spawn(async move {
            let metodo = npc_clone
                .npc_publication_contrato
                .method::<_, (LensType, Address, u8)>(
                    "getPublicationPredictByNPC",
                    npc_clone.npc.billetera.parse::<Address>().unwrap(),
                );

            match metodo {
                Ok(call) => {
                    let result: Result<
                        (LensType, Address, u8),
                        ethers::contract::ContractError<
                            SignerMiddleware<Arc<Provider<Http>>, LocalWallet>,
                        >,
                    > = call.call().await;

                    match result {
                        Ok((eleccion, artista, pagina)) => {
                            let mut prompt = "";
                            let mut imagen: Option<String> = None;
                            let locale = "en";

                            if eleccion == LensType::Autograph {
                                let metodo =
                                    npc_clone.autograph_data_contrato.method::<_, Vec<u128>>(
                                        "getArtistCollectionsAvailable",
                                        artista.clone(),
                                    );

                                match metodo {
                                    Ok(llama) => {
                                        let result: Result<
                                            Vec<u128>,
                                            ethers::contract::ContractError<
                                                SignerMiddleware<Arc<Provider<Http>>, LocalWallet>,
                                            >,
                                        > = llama.call().await;

                                        match result {
                                            Ok(mut colecciones) => {
                                                let rng = Arc::new(TokioMutex::new(
                                                    ChaCha12Rng::from_entropy(),
                                                ));

                                                if let Some(&mut numero_aleatorio) = colecciones
                                                    .choose_mut(&mut *rng.clone().lock().await)
                                                {
                                                    let metodo = npc_clone
                                                        .autograph_data_contrato
                                                        .method::<_, u8>(
                                                        "getCollectionGallery",
                                                        numero_aleatorio,
                                                    );

                                                    match metodo {
                                                        Ok(llama) => {
                                                            let resultado: Result<
                                                                u8,
                                                                ethers::contract::ContractError<
                                                                    SignerMiddleware<
                                                                        Arc<Provider<Http>>,
                                                                        LocalWallet,
                                                                    >,
                                                                >,
                                                            > = llama.call().await;

                                                            match resultado {
                                                                Ok(galeria) => {
                                                                    let metodo = npc_clone
                                                                        .autograph_data_contrato
                                                                        .method::<_, String>(
                                                                            "getCollectionURIByGalleryId",
                                                                            (numero_aleatorio, galeria),
                                                                        );

                                                                    match metodo {
                                                                        Ok(llama) => {
                                                                            let resultado: Result<
                                                                                String,
                                                                                ethers::contract::ContractError<
                                                                                    SignerMiddleware<Arc<Provider<Http>>, LocalWallet>,
                                                                                >,
                                                                            > = llama.call().await;

                                                                            match resultado {
                                                                                Ok(uri) => {
                                                                                    imagen =
                                                                                        Some(uri);
                                                                                }
                                                                                Err(e) => {
                                                                                    eprintln!(
                                                                                        "Error al obtener la URI de la imagen: {}",
                                                                                        e
                                                                                    );
                                                                                }
                                                                            }
                                                                        }
                                                                        Err(e) => {
                                                                            println!("Un error de ABI {}", e);
                                                                        }
                                                                    }
                                                                }
                                                                Err(e) => {
                                                                    eprintln!(
                                                                        "Error al obtener la galeria: {}",
                                                                        e
                                                                    );
                                                                }
                                                            }
                                                        }
                                                        Err(e) => {
                                                            println!("Un error de ABI {}", e);
                                                        }
                                                    }
                                                } else {
                                                    println!("El array está vacío.");
                                                }
                                            }
                                            Err(e) => {
                                                eprintln!(
                                                    "Error al obtener las colecciones: {}",
                                                    e
                                                );
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        println!("Un error de ABI {}", e);
                                    }
                                }
                            } else if eleccion == LensType::Catalog {
                                let metodo = npc_clone
                                    .autograph_data_contrato
                                    .method::<_, String>("getAutographPage", pagina);

                                match metodo {
                                    Ok(llama) => {
                                        let resultado: Result<
                                            String,
                                            ethers::contract::ContractError<
                                                SignerMiddleware<Arc<Provider<Http>>, LocalWallet>,
                                            >,
                                        > = llama.call().await;

                                        match resultado {
                                            Ok(uri) => {
                                                imagen = Some(uri);
                                            }
                                            Err(e) => {
                                                eprintln!(
                                                    "Error al obtener la página del catálogo: {}",
                                                    e
                                                );
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        println!("Un error de ABI {}", e);
                                    }
                                }
                            } else {
                                if eleccion == LensType::Comment {
                                    prompt = "make me a comment for twitter";
                                } else {
                                    prompt = "make me a post for twitter";
                                }
                            }

                            match llama.llamar_llama(prompt).await {
                                Ok(mensaje) => {
                                    match npc_clone
                                        .formatear_pub(&mensaje, locale, imagen.as_deref())
                                        .await
                                    {
                                        Ok(publicacion_id) => {
                                            let method = npc_clone
                                                .npc_publication_contrato
                                                .method::<_, H256>(
                                                    "registerPublication",
                                                    RegisterPub {
                                                        _artist: artista,
                                                        _profileId: npc_clone.npc.perfil_id,
                                                        _pubId: publicacion_id + 1,
                                                        _pageNumber: pagina,
                                                        _lensType: eleccion,
                                                    },
                                                );

                                            match method {
                                                Ok(call) => {
                                                    let FunctionCall { tx, .. } = call;

                                                    if let Some(tx_request) = tx.as_eip1559_ref() {
                                                        let gas_price =
                                                            U256::from(200_000_000_000u64);
                                                        let max_priority_fee =
                                                            U256::from(10_000_000_000u64);
                                                        let gas_limit = U256::from(200_000);
                                                        let cliente = npc_clone
                                                            .npc_publication_contrato
                                                            .client()
                                                            .clone();
                                                        let nonce = cliente
                                                            .clone()
                                                            .get_transaction_count(
                                                                npc_clone
                                                                    .npc
                                                                    .billetera
                                                                    .parse::<Address>()
                                                                    .unwrap(),
                                                                None,
                                                            )
                                                            .await
                                                            .expect("Error al recuperar el nonce");

                                                        let req = Eip1559TransactionRequest {
                                                            from: Some(
                                                                npc_clone
                                                                    .npc
                                                                    .billetera
                                                                    .parse::<Address>()
                                                                    .unwrap(),
                                                            ),
                                                            to: Some(NameOrAddress::Address(
                                                                NPC_PUBLICATION
                                                                    .parse::<Address>()
                                                                    .unwrap(),
                                                            )),
                                                            gas: Some(gas_limit),
                                                            value: tx_request.value,
                                                            data: tx_request.data.clone(),
                                                            max_priority_fee_per_gas: Some(
                                                                max_priority_fee,
                                                            ),
                                                            max_fee_per_gas: Some(
                                                                gas_price + max_priority_fee,
                                                            ),
                                                            nonce: Some(nonce),
                                                            chain_id: Some(
                                                                Chain::PolygonAmoy.into(),
                                                            ),
                                                            ..Default::default()
                                                        };

                                                        let cliente = npc_clone
                                                            .npc_publication_contrato
                                                            .client();
                                                        let pending_tx = cliente
                                                            .send_transaction(req, None)
                                                            .await
                                                            .map_err(|e| println!("Error al enviar la transacción: {}", e))
                                                            .expect("Error fatal al enviar la transacción");
                                                        let tx_hash = pending_tx
                                                            .confirmations(1)
                                                            .await
                                                            .map_err(|e| {
                                                                println!(
                                                                    "Error con la transacción: {}",
                                                                    e
                                                                )
                                                            })
                                                            .expect("Error con la transacción");
                                                        // println!(
                                                        //     "Transacción enviada con hash: {:?}",
                                                        //     tx_hash
                                                        // );
                                                    }
                                                }
                                                Err(e) => {
                                                    eprintln!(
                                                        "Error al registrar la publicación: {}",
                                                        e
                                                    );
                                                }
                                            }
                                        }
                                        Err(e) => {
                                            eprintln!("Error al formatear la publicación: {}", e);
                                        }
                                    }
                                }
                                Err(e) => {
                                    eprintln!("Error con la generación del mensaje: {:?}", e);
                                }
                            }
                        }
                        Err(e) => {
                            eprintln!("Error al obtener la predicción de la publicación: {}", e);
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Error al crear el método: {}", e);
                }
            }
        });
    }

    async fn formatear_pub(
        &self,
        mensaje: &str,
        locale: &str,
        imagen: Option<&str>,
    ) -> Result<U256, Box<dyn Error + Send + Sync>> {
        let mut imagen_url: Option<Imagen> = None;
        let mut enfoque = "TEXT_ONLY".to_string();
        let mut schema =
            "https://json-schemas.lens.dev/publications/text-only/3.0.0.json".to_string();

        if let Some(base64_imagen) = imagen {
            enfoque = String::from("IMAGE");
            schema = "https://json-schemas.lens.dev/publications/image/3.0.0.json".to_string();

            if base64_imagen.contains("ipfs://") {
                let opcion = Imagen {
                    tipo: "image/png".to_string(),
                    item: String::from(base64_imagen),
                };
                imagen_url = Some(opcion);
            } else {
                match subir_ipfs_imagen(base64_imagen).await {
                    Ok(cid) => {
                        let opcion = Imagen {
                            tipo: "image/png".to_string(),
                            item: format!("ipfs://{}", cid.Hash),
                        };
                        imagen_url = Some(opcion);
                    }
                    Err(e) => {
                        eprintln!("Error al subir la imagen: {}", e);
                    }
                }
            }
        }

        let publicacion = Publicacion {
            schema,
            lens: Contenido {
                mainContentFocus: enfoque,
                title: mensaje.chars().take(20).collect(),
                content: mensaje.to_string(),
                appId: "npcstudio".to_string(),
                id: Uuid::new_v4().to_string(),
                hideFromFeed: false,
                locale: locale.to_string(),
                tags: vec!["npcStudio".to_string(), self.escena.clone()],
                image: imagen_url,
            },
        };

        let publicacion_json = to_string(&publicacion)?;

        let contenido = match subir_ipfs(publicacion_json).await {
            Ok(con) => con.Hash,
            Err(e) => {
                eprintln!("Error al subir la publicacion al IPFS: {}", e);
                return Err(
                    Box::new(CustomError::new(&e.to_string())) as Box<dyn Error + Send + Sync>
                );
            }
        };

        let resultado = self.enviar_mensaje(contenido).await?;

        Ok(resultado)
    }

    async fn enviar_mensaje(
        &self,
        contenido: String,
    ) -> Result<U256, Box<dyn Error + Send + Sync>> {
        let mensaje = Pub {
            profileId: self.npc.perfil_id,
            contentURI: String::from("ipfs://") + &contenido,
            actionModules: vec![],
            actionModulesInitDatas: vec![],
            referenceModule: "0x0000000000000000000000000000000000000000"
                .parse::<Address>()
                .unwrap(),
            referenceModuleInitData: Bytes::from(vec![0u8; 1]),
        };

        let method = self
            .lens_hub_contrato
            .method::<_, U256>("post", (Token::Tuple(mensaje.into_tokens()),))?;

        let FunctionCall { tx, .. } = method;

        if let Some(tx_request) = tx.as_eip1559_ref() {
            let cliente = self.lens_hub_contrato.client().clone();
            let gas_price = U256::from(500_000_000_000u64);
            let max_priority_fee = U256::from(20_000_000_000u64);
            let gas_limit = U256::from(300_000);
            let tx_cost = gas_limit * gas_price + max_priority_fee;

            if cliente
                .clone()
                .get_balance(self.npc.billetera.parse::<Address>().unwrap(), None)
                .await?
                < tx_cost
            {
                return Err(Box::new(CustomError::new("Fondos insuficientes para gas")));
            }

            let req = Eip1559TransactionRequest {
                from: Some(self.npc.billetera.parse::<Address>().unwrap()),
                to: Some(NameOrAddress::Address(
                    LENS_HUB_PROXY.parse::<Address>().unwrap(),
                )),
                gas: Some(gas_limit),
                value: tx_request.value,
                // nonce: Some(
                //     cliente
                //         .clone()
                //         .get_transaction_count(self.npc.billetera.parse::<Address>().unwrap(), None)
                //         .await?
                // ),
                data: tx_request.data.clone(),
                max_priority_fee_per_gas: Some(max_priority_fee),
                max_fee_per_gas: Some(gas_price + max_priority_fee),
                chain_id: Some(Chain::PolygonAmoy.into()),
                ..Default::default()
            };
            let cliente = self.lens_hub_contrato.client().clone();
            let pending_tx = cliente.send_transaction(req, None).await?;
            let tx_hash = pending_tx.confirmations(1).await?;
            // println!("Transacción enviada con hash: {:?}", tx_hash);

            let resultado = lens::hacer_consulta(&format!("0x0{:x}", &self.npc.perfil_id))
                .await
                .map_err(|e| {
                    Box::new(CustomError::new(&e.to_string())) as Box<dyn Error + Send + Sync>
                })?;

            Ok(resultado)
        } else {
            Err(Box::new(CustomError::new("Error en Transacción")) as Box<dyn Error + Send + Sync>)
        }
    }
}
