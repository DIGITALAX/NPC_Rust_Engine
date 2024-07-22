use crate::{bib::{lens, types::{
    Comment, Contenido, Coordenada, CustomError, Estado, GameTimer, Imagen, LensType, Llama, Mapa, Mirror, Movimiento, NPCAleatorio, Pub, Publicacion, RegisterPub, Silla, Sprite, Talla
}, utils::{between, subir_ipfs, subir_ipfs_imagen}}, TokensAlmacenados, ISO_CODES, LENS_HUB_PROXY, NPC_PUBLICATION};
use abi::{Token, Tokenize};
use ethers::{prelude::*, types::{Address, Bytes, U256}};
use pathfinding::prelude::astar;
use serde_json::to_string;
use rand::{prelude::{SliceRandom,IteratorRandom}, thread_rng, Rng};
use tokio::runtime::Handle;
use std::{str::FromStr, error::Error,
    sync::{Arc, Mutex},  marker::{Send,Sync}};
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
        manija: Handle    ) -> Self {
        let (lens_hub_contrato, autograph_data_contrato, npc_publication_contrato) =
            lens::inicializar_contrato(&sprite.etiqueta.to_string());
            lens::inicializar_api();

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
            manija,
            tokens: None
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
            self.ultimo_tiempo_comprobacion = self.npc.publicacion_reloj;
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
                        eprintln!(
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

        if sillas_disponibles.clone().count() >0 {

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
    
            let sillas_ocupadas = Arc::clone(&self.sillas_ocupadas);
            let silla_aleatoria_etiqueta = silla_aleatoria.etiqueta.clone();
            self.reloj_juego.set_timeout(
                move || {
                    let mut sillas_taken = sillas_ocupadas.lock().unwrap();
                    sillas_taken.retain(|silla| silla.etiqueta != silla_aleatoria_etiqueta);
                },
                (bt / 600.0) as u64,
            );
        } else {
     
            self.silla_cerca = Some(Coordenada {
                x: self.npc.x as i32,
                y: self.npc.y as i32,
            });
        }
        self.npc.x = self.silla_cerca.unwrap().x as f32;
        self.npc.y = self.silla_cerca.unwrap().y as f32;
      
        self.contador = 0.0;
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
        if self.caminos.len() > 200 {
            self.caminos = self.caminos.split_off(self.caminos.len() - 200);
        }
    }

    fn comprobar_conversacion(&mut self)  {
        let llama = Llama;
        let mut npc_clone = Arc::new(self.clone());
       
        self.manija.spawn(async move {

            let tokens = lens::obtener_o_refrescar_tokens(&npc_clone.npc.etiqueta.to_string(), 
            npc_clone.npc.perfil_id
            , npc_clone.tokens.clone()
        
        )
            .await 
            .map_err(|e| Box::new(CustomError::new(&e.to_string())) as Box<dyn Error + Send + Sync  >);
        

match tokens {

    Ok (nuevos_tokens) => {
        Arc::get_mut(&mut npc_clone).unwrap().actualizar_tokens(nuevos_tokens.clone());

        let metodo = npc_clone
        .npc_publication_contrato
        .method::<_, (LensType, U256, u8, U256)>(
            "getPublicationPredictByNPC",
            npc_clone.npc.billetera.parse::<Address>().unwrap(),
        );

        match metodo {
            Ok(call) => {
                let resultado: Result<
                    (LensType, U256, u8, U256),
                    ethers::contract::ContractError<
                        SignerMiddleware<Arc<Provider<Http>>, LocalWallet>,
                    >,
                > = call.call().await;

                match resultado {
                    Ok((eleccion, coleccion_id, pagina, mut perfil_id)) => {
                        let mut prompt = "";
                        let mut imagen: Option<String> = None;
                        let mut locale =npc_clone.npc.prompt.idiomas.first().unwrap().to_string();
                        let limite_palabra = [10,50,100,500,700][thread_rng().gen_range(0..5)] ;
                        let etiquetas = ["with hashtags", "without hashtags"][thread_rng().gen_range(0..2)];
                        let mut galeria = 0;
                        let mut  comentario_perfil = U256::from(0);
                        let mut comentario_pub= U256::from(0);
                        let mut metadata_uri: String = String::from("");

                        if eleccion == LensType::Autograph {
                          
                           let metodo = npc_clone.autograph_data_contrato.method::<_, u16>("getCollectionGallery", coleccion_id);

                            match metodo {
                                Ok (llama) => {
                                    let resultado: Result<
                                    u16,
                                        ethers::contract::ContractError<
                                            SignerMiddleware<Arc<Provider<Http>>, LocalWallet>,
                                        >,
                                    > = llama.call().await;

                                    match resultado {
                                        Ok (galeria_id) => {

                                            galeria = galeria_id;

  
                                            let metodo = npc_clone.autograph_data_contrato.method::<_, String>("getCollectionURIByGalleryId", (coleccion_id, galeria));
                        

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
                                                        
                                                                if let Some(start_index) = uri.find("\"imagen\": \"ipfs://") {
                                                                    let ipfs_start = start_index + "\"imagen\": \"".len();
                                                                    if let Some(end_index) = uri[ipfs_start..].find('"') {
                                                                        let ipfs_uri = &uri[ipfs_start..ipfs_start + end_index];
                                                               

                                                                        imagen =
                                                                        Some(ipfs_uri.to_string());
                                                                    }else {
                                                                        let ipfs_uri = &uri[ipfs_start..];
                                                                        imagen = Some(ipfs_uri.to_string());
                                                                    }
                                                                }
                                                        }
                                                        Err(e) => {
                                                            eprintln!(
                                                                "Error al obtener la URI de la imagen: {}",
                                                                e
                                                            );
                                                            return;
                                                        }
            
                                                    }
                                                  
                                                }
                                                Err(e) => {
                                                    eprintln!("Un error de ABI {}", e);
                                                    return;
                                                }
                                            }




                                        },

                                        Err (e) => {
                                            eprintln!(
                                                "Error al obtener el Id de la galería: {}",
                                                e
                                            );
                                        }
                                    }
                                },

                                Err(e) => {
                                    eprintln!(
                                        "Error al obtener el Id de la galería: {}",
                                        e
                                    );
                                    return;
                                }
                            }


                         

                            
                        }  else if eleccion == LensType::Catalog {
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
                                            return;
                                        }
                                    }
                                }
                                Err(e) => {
                                    eprintln!("Un error de ABI {}", e);
                                    return;
                                }
                            }
                        } else {
                           let mut haz_pub = false;
                            if eleccion == LensType::Comment || eleccion == LensType::Mirror || eleccion == LensType::Quote {
                              
                               if perfil_id == U256::from(0)
{  

let mut rng = thread_rng();
if let Some(&npc_id) = npc_clone.npc.prompt.amigos.choose(&mut rng) {
    perfil_id = npc_id;
}

}
                               
let (contenido, perfil, publicacion, metadata) = lens::coger_comentario(&format!("0x0{:x}",

perfil_id

))
    .await
    .map_err(|e| Box::new(CustomError::new(&e.to_string())) as Box<dyn Error + Send + Sync  >)
    .expect("Error al encontrar el comentario");


    if metadata != ""  {
        let mut idiomas_para_prompt = npc_clone.npc.prompt.idiomas.clone();
        if coleccion_id != U256::from(0) && galeria != 0 {
            let metodo = npc_clone
            .autograph_data_contrato
            .method::<_, Vec<String>>("getCollectionLanguagesByGalleryId", (coleccion_id, galeria));

            match metodo {


              
                Ok(llama) => {
              
                    let resultado: Result<
                    Vec<String>,
                        ethers::contract::ContractError<
                            SignerMiddleware<Arc<Provider<Http>>, LocalWallet>,
                        >,
                    > = llama.call().await;

                    match resultado {
                        Ok(idiomas) => idiomas_para_prompt = idiomas,
                        Err(e) => {println!("Error al llamar los idiomas {}", e);}

                
                    }
                   

                },
                Err(e) => {println!("Error al llamar los idiomas {}", e);}
            }


        }

                
        let mut rng = thread_rng();
        if let Some(idioma_aleatorio) = idiomas_para_prompt.choose(&mut rng) {
            locale = idioma_aleatorio.to_string();
        }

            let new_prompt = {
                let mut temp_prompt = "Respond to this post in the language of ".to_string();
                temp_prompt.push_str(&locale);
                temp_prompt.push_str("and with a word limit of ");
                temp_prompt.push_str(&limite_palabra.to_string());
                temp_prompt.push_str("and");
                temp_prompt.push_str(&etiquetas);
                temp_prompt.push_str("with a comment and in the style of a person with this personality and backstory: ");
                temp_prompt.push_str(&npc_clone.npc.prompt.personalidad.to_string());
                temp_prompt.push_str(". Remember three very very important rules: 1. Only give me the comment in your reply, nothing more. Do not tell me that the comment is there, only give the comment as it will go directly to post. For example NEVER write 'Here's the social media post:' only give me the comment. 2. REMEMBER NEVER EVER NEVER EVER write a translation or a pronunciation, only I want the language specified above in the comment NOTHING ELSE. 3. If the language chosen above is not english DO NOT rewrite the comment in english, I only want that language. \n\npost :\n\n");
                temp_prompt.push_str(&contenido);
                temp_prompt
            };

           metadata_uri = metadata;
            comentario_perfil = perfil;
            comentario_pub = publicacion;
            prompt = Box::leak(Box::new(new_prompt)).as_str();
    } else {
        haz_pub = true;
    }





                           

                            } else {
                                haz_pub = true;
                            } 



                            if haz_pub {
                                let new_prompt = {
                                    let mut temp_prompt = "Make me a post for social media in the language of ".to_string();
                                    temp_prompt.push_str(&locale);
                                    temp_prompt.push_str("and with a word limit of ");
                                    temp_prompt.push_str(&limite_palabra.to_string());
                                    temp_prompt.push_str("and");
                                    temp_prompt.push_str(&etiquetas);
                                    temp_prompt.push_str("and in the style of a person with this personality and backstory: ");
                                    temp_prompt.push_str(&npc_clone.npc.prompt.personalidad.to_string());
                                    temp_prompt.push_str(". Remember three very very important rules: 1. Only give me the post in your reply, nothing more. Do not tell me that the post is there, only give the post as it will go directly to post.  For example NEVER write 'Here's the social media post:' only give me the post. 2. REMEMBER NEVER EVER NEVER EVER write a translation or a pronunciation, only I want the language specified above in the post NOTHING ELSE. 3. If the language chosen above is not english DO NOT rewrite the post in english, I only want that language.");
                                    temp_prompt
                                };

                                prompt = Box::leak(Box::new(new_prompt)).as_str();
                            }
                        }

                        match llama.llamar_llama(prompt).await {
                            Ok(mensaje) => {
                                match npc_clone
                                    .formatear_pub(metadata_uri,&mensaje, &locale, imagen.as_deref(), eleccion.clone(), comentario_perfil, comentario_pub)
                                    .await
                                {
                                    Ok(publicacion_id) => {
                                        let method = npc_clone
                                            .npc_publication_contrato
                                            .method::<_, H256>(
                                                "registerPublication",
                                                RegisterPub {
                                                    _collection: coleccion_id,
                                                    _profileId: perfil_id,
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
                                                        .map_err(|e| Box::new(CustomError::new(&e.to_string())) as Box<dyn Error + Send + Sync  >)
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
                                                            Chain::Polygon.into(),
                                                        ),
                                                        ..Default::default()
                                                    };

                                                    let cliente = npc_clone
                                                        .npc_publication_contrato
                                                        .client();
                                                    let pending_tx = cliente
                                                        .send_transaction(req, None)
                                                        .await
                                                       
                                                    .map_err(|e| Box::new(CustomError::new(&e.to_string())) as Box<dyn Error + Send + Sync  >)
                                                    .expect("Error fatal al enviar la transacción");
                                                    let tx_hash = pending_tx
                                                        .confirmations(1)
                                                        .await
                                                  
                                                    .map_err(|e| Box::new(CustomError::new(&e.to_string())) as Box<dyn Error + Send + Sync  >)
                                                    .expect("Error con la transacción");
                                                    println!(
                                                        "Transacción enviada con hash: {:?}",
                                                        tx_hash
                                                    );
                                                }
                                            }
                                            Err(e) => {
                                                eprintln!(
                                                    "Error al registrar la publicación: {}",
                                                    e
                                                );
                                                return;
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        eprintln!("Error al formatear la publicación: {}", e);
                                        return;
                                    }
                                }
                            }
                            Err(e) => {
                                eprintln!("Error con la generación del mensaje: {:?}", e);
                                return;
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("Error al obtener la predicción de la publicación: {}", e);
                        return;
                    }
                }
            }
            Err(e) => {
                eprintln!("Error al crear el método: {}", e);
                return;
            }
        }
    }

    Err(e) => {
        eprintln!(
            "Error al conectarse a Lens: {}",
            e
        );
        return;
    }


}

           
           
        });
    }

    async fn formatear_pub(
        &self,
        metadata_uri: String,
        mensaje: &str,
        locale: &str,
        imagen: Option<&str>,
        lens_tipo: LensType,
        comentario_perfil: U256,
        comentario_pub: U256
    ) -> Result<U256, Box<dyn Error + Send + Sync  >>{
        let mut imagen_url: Option<Imagen> = None;
        let mut enfoque = "TEXT_ONLY".to_string();
        let mut schema =
            "https://json-schemas.lens.dev/publications/text-only/3.0.0.json".to_string();

        let numero_umbral: f64 = thread_rng().gen();

        if imagen.is_none() &&  imagen_url.is_none() && matches!(lens_tipo, LensType::Publication) && numero_umbral < 0.2 {

            let mut rng = thread_rng();
            let mut opciones = self.npc.prompt.imagenes.lock().unwrap();
            if opciones.len() > 0
 {
    if let Some(index) = (0..opciones.len()).choose(&mut rng) {
        let imagen = opciones.remove(index);
        imagen_url = Some(Imagen {
            tipo: "image/png".to_string(),
            item:format!("ipfs://{}",imagen),
        });
    }
 }            


        }



        if let Some(base64_imagen) = imagen {

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
                        return Err(Box::new(CustomError::new(&e.to_string())) as Box<dyn Error + Send + Sync  >);
                       
             

                    }
                }
            }
        }


        if let Some(_) = imagen_url.as_ref() {
            enfoque = String::from("IMAGE");
            schema = "https://json-schemas.lens.dev/publications/image/3.0.0.json".to_string();
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
                locale: ISO_CODES.get(locale).unwrap().to_string(),
                tags: vec!["npcStudio".to_string(), self.escena.clone()],
                image: imagen_url,
            },
        };

        let publicacion_json = to_string(&publicacion)?;

        let contenido = match subir_ipfs(publicacion_json).await {
            Ok(con) => con.Hash,
            Err(e) => {
                eprintln!("Error al subir la publicacion al IPFS: {}", e);
                return Err(Box::new(CustomError::new(&e.to_string())) as Box<dyn Error + Send + Sync  >);
                       

            }
        };

        match self.enviar_mensaje(contenido, metadata_uri, lens_tipo, comentario_perfil, comentario_pub).await {
            Ok(resultado) => {
               return Ok(resultado)
            }
            Err(e) => {
                eprintln!("Error al enviar el mensaje: {:?}", e);
                Err(Box::new(CustomError::new(&e.to_string())) as Box<dyn Error + Send + Sync  >)
                       
            }
        }


    }

    async fn enviar_mensaje(
        &self,
        contenido: String,
        metadata_uri: String,
        lens_tipo: LensType,
        comentario_perfil: U256,
        comentario_pub: U256
    ) -> Result<U256, Box<dyn Error + Send + Sync>> {
        let method;
        let res: Result<String, Box<dyn Error + std::marker::Send + Sync>>;
       if lens_tipo == LensType::Comment || lens_tipo == LensType::Quote {

        let mensaje = Comment {
            profileId: 
            self.npc.perfil_id,
            contentURI: String::from("ipfs://") + &contenido,
            pointedProfileId: comentario_perfil,
            pointedPubId: comentario_pub,
            referrerProfileIds: vec![],
            referrerPubIds: vec![],
            referenceModuleData: Bytes::from(vec![0u8; 1]),
            actionModules: vec![],
            actionModulesInitDatas: vec![],
            referenceModule: "0x0000000000000000000000000000000000000000"
                .parse::<Address>()
                .unwrap(),
            referenceModuleInitData: Bytes::from(vec![0u8; 1]),
        };

        let mut funcion = "comment";
 

        if lens_tipo == LensType::Quote {
            funcion = "quote";
            res = lens::hacer_cita(&self.npc.etiqueta, &format!("0x0{:x}-0x{:02x}", comentario_perfil, comentario_pub)
                
                
             
            , String::from("ipfs://") + &contenido, &self.tokens.as_ref().unwrap().tokens.access_token).await.map_err(|e| {
                Box::new(CustomError::new(&e.to_string())) as Box<dyn Error + Send + Sync>
            });
        } else {

         res = lens::hacer_comentario(&self.npc.etiqueta, &format!("0x0{:x}-0x{:02x}", comentario_perfil, comentario_pub), String::from("ipfs://") + &contenido, &self.tokens.as_ref().unwrap().tokens.access_token).await.map_err(|e| {
            Box::new(CustomError::new(&e.to_string())) as Box<dyn Error + Send + Sync>
        });

        }




         method = self
        .lens_hub_contrato
        .method::<_, U256>(funcion, (Token::Tuple(mensaje.into_tokens()),))?;

       }else if lens_tipo == LensType::Mirror {

        let mensaje = Mirror {
            profileId: 
            self.npc.perfil_id,
            metadataURI: metadata_uri,
            pointedProfileId: comentario_perfil,
            pointedPubId: comentario_pub,
            referrerProfileIds: vec![],
            referrerPubIds: vec![],
            referenceModuleData: Bytes::from(vec![0u8; 1]),
         
        };

         method = self
        .lens_hub_contrato
        .method::<_, U256>("mirror", (Token::Tuple(mensaje.into_tokens()),))?;

        res = lens::hacer_mirror(&self.npc.etiqueta, &format!("0x0{:x}-0x{:02x}", comentario_perfil, comentario_pub), &self.tokens.as_ref().unwrap().tokens.access_token).await.map_err(|e| {
            Box::new(CustomError::new(&e.to_string())) as Box<dyn Error + Send + Sync>
        });


       }
       
       
       
       
       else {
        let mensaje = Pub {
            profileId:
            self.npc.perfil_id,
            contentURI: String::from("ipfs://") + &contenido,
            actionModules: vec!["0x34A437A91415C36712B0D912c171c74595Be437d" .parse::<Address>()
            .unwrap()],
            actionModulesInitDatas: vec![
Bytes::from_str("0x000000000000000000000000185b529b421ff60b0f2388483b757b39103cfcb1000000000000000000000000000000000000000000000000000000000000004000000000000000000000000000000000000000000000000000000000000000e00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000")?,
              

            ],
            referenceModule: "0x0000000000000000000000000000000000000000"
                .parse::<Address>()
                .unwrap(),
            referenceModuleInitData: Bytes::from(vec![0u8; 1]),
        };

         method = self
        .lens_hub_contrato
        .method::<_, U256>("post", (Token::Tuple(mensaje.into_tokens()),))?;

        res = lens::hacer_publicacion(&self.npc.etiqueta,  String::from("ipfs://") + &contenido, &self.tokens.as_ref().unwrap().tokens.access_token).await.map_err(|e| {
            Box::new(CustomError::new(&e.to_string())) as Box<dyn Error + Send + Sync>
        });
       } 


       if res.unwrap() != "RelaySuccess" {
    
        let FunctionCall { tx, .. } = method;

        if let Some(tx_request) = tx.as_eip1559_ref() {
            let cliente = self.lens_hub_contrato.client().clone();
            let gas_price = U256::from(500_000_000_000u64);
            let max_priority_fee = U256::from(20_000_000_000u64);
            let gas_limit = U256::from(300_000);
            let tx_cost = gas_limit * gas_price + max_priority_fee;

            if cliente
            .clone()
            .get_balance(
                
                
                self.npc.billetera.parse::<Address>().unwrap()
                
           
                , None)
            .await?
            < tx_cost
        {
            return Err(Box::new(CustomError::new("Fondos insuficientes para gas")));
        }

            let req = Eip1559TransactionRequest {
                from: Some(
            
                    
                    self.npc.billetera.parse::<Address>().unwrap()
                ),
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
                chain_id: Some(Chain::Polygon.into()),
                ..Default::default()
            };
            let cliente = self.lens_hub_contrato.client().clone();
            let pending_tx = cliente.send_transaction(req, None).await.map_err(|e| {
                Box::new(CustomError::new(&e.to_string())) as Box<dyn Error + Send + Sync  >
            }).expect("Error al enviar la transacción");
            
        let tx_hash = pending_tx.confirmations(1).await.map_err(|e| {
            Box::new(CustomError::new(&e.to_string())) as Box<dyn Error + Send + Sync  >
        }).expect("Error con la transacción");
        
            println!("Transacción enviada con hash: {:?}", tx_hash);


        } else {
            return Err(Box::new(CustomError::new("Error en Transacción")) as Box<dyn Error + Send + Sync  >)


        }
    }
 

    let resultado = lens::hacer_consulta(&format!("0x0{:x}", 

    
    &self.npc.perfil_id

))
    .await
    .map_err(|e| {
        Box::new(CustomError::new(&e.to_string())) as Box<dyn Error + Send + Sync>
    })?;

Ok(resultado)
    }

    pub fn actualizar_tokens(&mut self, nuevos_tokens: TokensAlmacenados) {
        self.tokens = Some(nuevos_tokens);
    }
}

