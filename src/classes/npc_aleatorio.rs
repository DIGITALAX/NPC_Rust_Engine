use crate::{bib::{lens, types::{
    Comment, Contenido, Coordenada, CustomError, Estado, GameTimer, Imagen, LensType, Llama, Mapa, Mirror, Movimiento, NPCAleatorio, Pub, Publicacion, RegisterPub, Silla, Sprite, Talla
}, utils::{between, from_hex_string, subir_ipfs, subir_ipfs_imagen}}, Boudica, EstadoNPC, LlamaOpciones, LlamaRespuesta, MetadataAttribute, PublicacionPrediccion, TokensAlmacenados, ISO_CODES, ISO_CODES_PROMPT, LENS_HUB_PROXY, NPC_PUBLICATION};
use abi::{Token, Tokenizable, Tokenize};
use chrono::Utc;
use ethers::{prelude::*, types::{Address, Bytes, U256}};
use k256::pkcs8::der::asn1::Null;
use pathfinding::prelude::astar;
use serde_json::{from_str, to_string, Value};
use rand::{prelude::{IteratorRandom, SliceRandom}, random, thread_rng, Rng};
use tokio::{runtime::Handle, sync::RwLock};
use std::{collections::HashSet, error::Error, marker::{Send,Sync}, str::FromStr, sync::{Arc, Mutex}};
use uuid::Uuid;
use reqwest::blocking;

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
            ultimo_tiempo_mencion: 0,
            lens_hub_contrato,
            autograph_data_contrato,
            npc_publication_contrato,
            escena,
            manija,
            tokens: None,
            estado: Arc::new(RwLock::new(EstadoNPC::Inactivo)),
            ultima_mencion_procesada: Arc::new(RwLock::new(Utc::now())),
            menciones_procesadas: Arc::new(RwLock::new(HashSet::new())),
            boudica: false,
            llama_recibido: None
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

        if self.ultimo_tiempo_mencion > 0 {
            self.ultimo_tiempo_mencion -= delta_time;
        }

        if self.ultimo_tiempo_comprobacion < self.npc.publicacion_reloj {
            self.ultimo_tiempo_comprobacion += delta_time;
        }

        if self.ultimo_tiempo_comprobacion >= self.npc.publicacion_reloj && self.llama_recibido.is_none() {
            self.ultimo_tiempo_comprobacion = 0;
            self.comprobar_conversacion();
        }

        // if self.ultimo_tiempo_comprobacion <= 0 && self.llama_recibido.is_none() {
        //     self.ultimo_tiempo_comprobacion = self.npc.publicacion_reloj;
        //     self.comprobar_conversacion();

          
           
        // }

        if let Some(datos) = self.llama_recibido.take() {
            self.procesar_llama(&datos); 
        }
        

        // if self.ultimo_tiempo_mencion <= 0 {
        //     self.ultimo_tiempo_mencion = 36000000;
        //     self.comprobar_menciones();
        // }
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
        let sillas_disponibles: Vec<&Silla> = self.sillas.iter().filter(|silla| {
            !self
                .sillas_ocupadas
                .lock()
                .unwrap()
                .iter()
                .any(|silla_ocupada| silla_ocupada.etiqueta == silla.etiqueta)
        }).collect();

        if !sillas_disponibles.is_empty() {
            let silla_aleatoria = sillas_disponibles[thread_rng().gen_range(0..sillas_disponibles.len())];


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

            let mut prompt = "";
            let mut imagen: Option<String> = None;
            let mut locale =npc_clone.npc.prompt.idiomas.first().unwrap().to_string();
            let limite_palabra = [100,200,350][thread_rng().gen_range(0..3)] ;
            let etiquetas = [" and at the end include some hashtags. ", " and do not include any hashtags. "][thread_rng().gen_range(0..2)];
            let mut galeria = 0;
            let mut  comentario_perfil = U256::from(0);
            let mut comentario_pub= U256::from(0);
            let mut metadata_uri: String = String::from("");



match tokens {

    Ok (nuevos_tokens) => {
        Arc::get_mut(&mut npc_clone).unwrap().actualizar_tokens(nuevos_tokens.clone());

        let metodo = npc_clone
        .npc_publication_contrato
        .method::<_, (LensType, U256, u8, U256)>(
            "getPublicationPredictByNPC",
            PublicacionPrediccion {
           _locale:     ISO_CODES.get(locale.as_str()).unwrap().to_string(),
           _npcWallet:    npc_clone.npc.billetera.parse::<Address>().unwrap(),
             _boudica:   npc_clone.boudica
            }
           
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
              

                        let mut haz_pub = false;
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


                        haz_pub = true;

                    }  else if eleccion == LensType::Catalog && !npc_clone.boudica && pagina != 0 {
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

                        haz_pub = true;
                    } 
                    else {
                        if npc_clone.boudica {


                            let metodo = npc_clone
                            .autograph_data_contrato
                            .method::<_, String>("getBoudicaPageText",      Boudica {_language: ISO_CODES.get(locale.as_str()).unwrap().to_string(),
                             _pageNumber: pagina});

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

                                        let respuesta = match blocking::get(&format!("https://asd.infura-ipfs.io/ipfs/{}", uri.split("ipfs://").nth(1).unwrap_or(""))) {
                                            Ok(resp) => match resp.text() {
                                                Ok(texto) => texto,
                                                Err(e) => {
                                                    eprintln!("Error al extraer el texto de la respuesta: {}", e);
                                                    return;
                                                }
                                            },
                                            Err(e) => {
                                                eprintln!("Error al recibir la respuesta del IPFS: {}", e);
                                                return;
                                            }
                                        };
                                        
                                        let parsed_json: Value = match serde_json::from_str(&respuesta) {
                                            Ok(json) => json,
                                            Err(e) => {
                                                eprintln!("Error al parsear la respuesta como JSON: {}", e);
                                                return;
                                            }
                                        };
                                        
                                        let contenido = match parsed_json.get("contenido") {
                                            Some(valor) => match valor.as_str() {
                                                Some(cadena) => cadena.to_string(),
                                                None => {
                                                    eprintln!("El campo 'contenido' no es una cadena");
                                                    return;
                                                }
                                            },
                                            None => {
                                                eprintln!("Error al extraer el campo 'contenido'");
                                                return;
                                            }
                                        };

                                        let new_prompt = {
                                            let mut temp_prompt = "You are a unique and quirky person named ".to_string();
                                            temp_prompt.push_str(npc_clone.npc.etiqueta.as_str());
                                            temp_prompt.push_str(" with the personality traits of ");
                                            temp_prompt.push_str(&npc_clone.npc.prompt.tono.join(", "));
                                            temp_prompt.push_str(". Your writing style is authentic, raw, playful, poetic and dense with ideas. You are currently adding guemara style notes to this publication.");
                                            temp_prompt.push_str("\n\nWrite a response that is less than ");
                                            temp_prompt.push_str(&limite_palabra.to_string());
                                            temp_prompt.push_str(" that adds guemara style comments to this content:\n\n");
                                            temp_prompt.push_str(&contenido);
                                            temp_prompt.push_str(". Write the response in the language of ");
                                            temp_prompt.push_str(
                                                ISO_CODES_PROMPT
                                                    .get(locale.as_str())
                                                    .map(|s| s.as_ref())
                                                    .unwrap_or("english")
                                            );
                                            temp_prompt.push_str(" and make sure to only use the alfabet of ");
                                            temp_prompt.push_str(
                                                ISO_CODES_PROMPT
                                                    .get(locale.as_str())
                                                    .map(|s| s.as_ref())
                                                    .unwrap_or("english")
                                            );
                                            temp_prompt.push_str(&etiquetas);                           
                                            temp_prompt.push_str(" Strive for writing that doesn't just communicate ideas but creates experiences. Your prose should leave readers slightly changed. Do not repeat back to me the prompt or finish my sentence if I asked for a non english language do not translate your response. Make sure to finish the prompt, don't cut it off.early.");
                                            temp_prompt
                                        };
                            
                                        print!("BOUDDICAAA");
                            
                                        prompt = Box::leak(Box::new(new_prompt)).as_str();
                                    }
                                    Err(e) => {
                                        eprintln!(
                                            "Error al obtener la página de Boudica: {}",
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

                           if eleccion == LensType::Comment || eleccion == LensType::Mirror || eleccion == LensType::Quote {





                         
                              if perfil_id == U256::from(0)
{  

let mut rng = thread_rng();
if let Some(&npc_id) = npc_clone.npc.prompt.amigos.choose(&mut rng) {
   perfil_id = npc_id;
}

}
                          
let (contenido, perfil, publicacion, metadata) = match lens::coger_comentario(&format!("0x0{:x}", perfil_id)).await {
    Ok(result) => result,
    Err(e) => {
        eprintln!("Error al encontrar el comentario: {}", e);
        return;
    }
};

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
               let mut temp_prompt = "You are a unique and quirky person named ".to_string();
               temp_prompt.push_str(npc_clone.npc.etiqueta.as_str());
               temp_prompt.push_str(" with the personality traits of ");
               temp_prompt.push_str(&npc_clone.npc.prompt.tono.join(", "));
               temp_prompt.push_str(". Your writing style is authentic, raw, playful, poetic and dense with ideas. You are currently having a conversation with another person that has been tested to have an IQ of 187+.");
               temp_prompt.push_str("\n\nWrite a response that is less than ");
               temp_prompt.push_str(&limite_palabra.to_string());
               temp_prompt.push_str(" that replies to this last comment ");
               temp_prompt.push_str(&contenido);
               temp_prompt.push_str(". Write the response in the language of ");
               temp_prompt.push_str(
                   ISO_CODES_PROMPT
                       .get(locale.as_str())
                       .map(|s| s.as_ref())
                       .unwrap_or("english")
               );
               temp_prompt.push_str(" and make sure to only use the alfabet of ");
               temp_prompt.push_str(
                   ISO_CODES_PROMPT
                       .get(locale.as_str())
                       .map(|s| s.as_ref())
                       .unwrap_or("english")
               );
               temp_prompt.push_str(&etiquetas);                           
               temp_prompt.push_str(" Strive for writing that doesn't just communicate ideas but creates experiences. Your prose should leave readers slightly changed. Do not repeat back to me the prompt or finish my sentence if I asked for a non english language do not translate your response.  Make sure to finish the prompt, don't cut it off.early.");
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


                         
                            
                        }
                        


                    }


                    if haz_pub {

                                                   let new_prompt = {
                                                       let mut temp_prompt =
                                                        "You are a unique and quirky person named ".to_string();
                                                       temp_prompt.push_str(npc_clone.npc.etiqueta.as_str());
                                                       temp_prompt.push_str(" with the personality traits of ");
                                                       temp_prompt.push_str(&npc_clone.npc.prompt.tono.join(", "));
                                                       temp_prompt.push_str(". Your writing style is authentic, raw, playful, poetic and dense with ideas. You are currently having a conversation with another person that has been tested to have an IQ of 187+.");
                                                       temp_prompt.push_str("\n\nWrite a response that is less than ");
                                                       temp_prompt.push_str(&limite_palabra.to_string());
                                                       temp_prompt.push_str(" about the topic of ");
                                                       let mut rng = rand::thread_rng();
                                                       if let Some(tema) = npc_clone.npc.prompt.temas.choose(&mut rng) {
                                                           temp_prompt.push_str(tema);
                                                       }
                                                       temp_prompt.push_str(". Write the response in the language of ");
                                                       temp_prompt.push_str(
                                                           ISO_CODES_PROMPT
                                                               .get(locale.as_str())
                                                               .map(|s| s.as_ref())
                                                               .unwrap_or("english")
                                                       );
                                                       temp_prompt.push_str(" and make sure to only use the alfabet of ");
                                                       temp_prompt.push_str(
                                                           ISO_CODES_PROMPT
                                                               .get(locale.as_str())
                                                               .map(|s| s.as_ref())
                                                               .unwrap_or("english")
                                                       );
                                                       temp_prompt.push_str(&etiquetas);                           
                                                       temp_prompt.push_str(" Strive for writing that doesn't just communicate ideas but creates experiences. Your prose should leave readers slightly changed. Do not repeat back to me the prompt or finish my sentence or confirm what I said. Only answer the prompt directly in the language and alfabet that I asked for. Do not translate your response into any other language. Make sure to finish the prompt, don't cut it off early.");
                                                       temp_prompt
                                                   };
                        
                                                   
                        
                                                   prompt = Box::leak(Box::new(new_prompt)).as_str();
                        
                                   
                                               }

                    match llama.llamar_llama(&npc_clone.npc.etiqueta, 
                        &npc_clone.escena,
                        metadata_uri,  
                        &locale, 
                        eleccion.clone(), 
                        // LensType::Autograph,
                        comentario_perfil, 
                        comentario_pub,
                        // U256::from(0),U256::from(0),0,
                        perfil_id,
                        coleccion_id,
                        pagina,
                        prompt, imagen.clone(), LlamaOpciones {
                        num_keep: 5,
                        seed: random::<i32>(),
                        num_predict: limite_palabra,
                        top_k: 20,
                        top_p: 0.9,
                        min_p: 0.0,
                        tfs_z: 0.5,
                        typical_p: 0.7,
                        repeat_last_n: 33,
                        temperature: 0.8,
                        repeat_penalty: 1.2,
                        presence_penalty: 1.5,
                        frequency_penalty: 1.0,
                        mirostat: 1,
                        mirostat_tau: 0.8,
                        mirostat_eta: 0.6,
                        penalize_newline: true,
                        numa: false,
                        num_tokens: limite_palabra,
                        num_batch: 2,
                        num_gpu: 18,
                        main_gpu: 0,
                        low_vram: false,
                        f16_kv: true,
                        vocab_only: false,
                        use_mmap: true,
                        use_mlock: false,
                        num_thread: 8
                    }).await {
                        Ok(_) => {

                           
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

        self.boudica = !self.boudica;
    }

    async fn formatear_pub(
        &self,
        metadata_uri: String,
        mensaje: LlamaRespuesta,
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

        let tags = if self.boudica {
            vec!["boudica".to_string()]
        } else {
            vec!["npcStudio".to_string(), self.escena.clone()]
        };

        let app_id = if self.boudica {
           "boudica".to_string()
        } else {
            "npcstudio".to_string()
        };
    
        let publicacion = Publicacion {
            schema,
            lens: Contenido {
                mainContentFocus: enfoque,
                title: mensaje.response.chars().take(20).collect(),
                content: mensaje.response.to_string(),
                appId: app_id,
                id: Uuid::new_v4().to_string(),
                hideFromFeed: false,
                locale: ISO_CODES.get(locale).unwrap().to_string(),
                tags,
                image: imagen_url,
                attributes:  vec![ 
                    MetadataAttribute {
                        key: "llm_info".to_string(),
                        tipo: "JSON".to_string(),
                        value: mensaje.json.to_string(),
                    }
                ].into()
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

        if lens_tipo == LensType::Mirror ||  lens_tipo == LensType::Quote || lens_tipo == LensType::Comment {
        match  lens::meGusta(&self.npc.etiqueta, &format!("0x0{:x}-0x{:02x}", comentario_perfil, comentario_pub), &self.tokens.as_ref().unwrap().tokens.access_token).await {
        Ok(_) => {},
        Err(e) => {       eprintln!("Error al gustar la publicacion al IPFS: {}", e);
        return Err(Box::new(CustomError::new(&e.to_string())) as Box<dyn Error + Send + Sync  >);}
       }
        }

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
            let max_priority_fee = U256::from(25_000_000_000u64);
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
            let pending_tx = match cliente.send_transaction(req, None).await {
                Ok(tx) => tx,
                Err(e) => {
                    println!("Error al enviar la transacción: {:?}", e);
                    return Err(Box::new(CustomError::new("Error al enviar la transacción")) as Box<dyn Error + Send + Sync  >)
                }
            };

        let tx_hash = match pending_tx.confirmations(1).await {
            Ok(hash) => hash,
            Err(e) => {
                println!("Error con la transacción: {:?}", e);
                return Err(Box::new(CustomError::new("Error con la transacción")) as Box<dyn Error + Send + Sync  >)
            }
        };
        
        
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


    // async fn comprobar_menciones(&self) -> Result<(), Box<dyn Error + Send + Sync>> {
    //     let mut npc_clone = Arc::new(self.clone());
       
    //     self.manija.spawn(async move {

    //         let tokens = lens::obtener_o_refrescar_tokens(&npc_clone.npc.etiqueta.to_string(), 
    //         npc_clone.npc.perfil_id
    //         , npc_clone.tokens.clone()
        
    //     )
    //         .await 
    //         .map_err(|e| Box::new(CustomError::new(&e.to_string())) as Box<dyn Error + Send + Sync  >); 
        
               
    //         let ultima_fecha = *self.ultima_mencion_procesada.read().await;
    //         let nuevas_menciones = buscar_menciones(&self.npc.etiqueta, &tokens.tokens.access_token, ultima_fecha).await?;
    
    //         for mencion in nuevas_menciones {
    //             self.procesar_mencion(mencion).await?;
    //         }
    
        
        
    //     });
       
       

    //     Ok(())
    // }

    pub fn llama_recibido(&mut self, datos_json: &String) {
        if self.llama_recibido.is_none() {
            self.llama_recibido = Some(datos_json.to_string());
        }
    }
    


    pub fn procesar_llama(&mut self, datos_json: &String) {

        let mut npc_clone = Arc::new(self.clone());
        let datos_clone = datos_json.clone();
    
        self.manija.spawn(async move {
            let tokens = lens::obtener_o_refrescar_tokens(
                &npc_clone.npc.etiqueta.to_string(),
                npc_clone.npc.perfil_id,
                npc_clone.tokens.clone(),
            )
            .await
            .map_err(|e| Box::new(CustomError::new(&e.to_string())) as Box<dyn Error + Send + Sync>);


match tokens {

    Ok (nuevos_tokens) => {
        Arc::get_mut(&mut npc_clone).unwrap().actualizar_tokens(nuevos_tokens.clone()); 
    
            if let Ok(parsed) = from_str::<Value>(&datos_clone) {
                let locale = parsed.get("locale").and_then(Value::as_str).unwrap_or("");
    
                let metadata_uri = parsed
                    .get("metadata_uri")
                    .and_then(Value::as_str)
                    .unwrap_or("") 
                    .to_string();
                
                    let comentario_perfil: U256 = match parsed
                    .get("comentario_perfil")
                    .and_then(Value::as_str)
                    .map(|val| from_hex_string(val)) {
                        Some(Ok(val)) => U256::from(val),
                        _ => U256::zero(),
                };
                let comentario_pub: U256 = match parsed
                .get("comentario_pub")
                .and_then(Value::as_str)
                .map(|val| from_hex_string(val)) {
                    Some(Ok(val)) => U256::from(val),
                    _ => U256::zero(),
            };
                    let coleccion_id: U256 = match parsed
                    .get("coleccion_id")
                    .and_then(Value::as_str)
                    .map(|val| from_hex_string(val)) {
                        Some(Ok(val)) => U256::from(val),
                        _ => U256::zero(),
                };

                let perfil_id: U256 = match parsed
                .get("perfil_id")
                .and_then(Value::as_str)
                .map(|val| from_hex_string(val)) {
                    Some(Ok(val)) => U256::from(val),
                    _ => U256::zero(),
            };
                
            
                let pagina: u8 = parsed
                    .get("pagina")
                    .and_then(Value::as_u64)
                    .unwrap_or(0)
                    .try_into()
                    .unwrap();  
                let eleccion = match parsed.get("eleccion")
                .and_then(|eleccion| eleccion.get("Uint"))
                .and_then(Value::as_str)
                .map(|val| from_hex_string(val)) {
                    Some(Ok(val)) => val as u8,
                    _ => 0,
            };

                let eleccion = match LensType::try_from(eleccion) {
                Ok(val) => val,
                Err(e) => {
                    eprintln!("Error al convertir valor a LensType: {:?}", e);
                    return;
                }
            };
            
            let valores = match parsed.get("mensaje") {
                Some(val) => val,
                None => {
                    eprintln!("Campo 'mensaje' no encontrado");
                    return;
                }
            };
                
                let json_clonado = valores.clone();
            

                let response = valores.get("output")
                .and_then(Value::as_str)
                .unwrap_or("") 
                .to_string();


let mensaje = LlamaRespuesta {
    response,
    json: json_clonado,
};


if mensaje.response == "" || mensaje.json.is_null(){
    eprintln!("Mensaje es null {:?}", mensaje);
return;
} else {
       
                match npc_clone
                    .formatear_pub(
                        metadata_uri,
                        mensaje.clone(),
                        locale,
                        parsed.get("image").and_then(Value::as_str), 
                        eleccion.clone(),
                        comentario_perfil,
                        comentario_pub,
                    )
                    .await
                {
                    Ok(publicacion_id) => {
                        let tensores = match subir_ipfs(serde_json::to_string(&mensaje.json).unwrap()).await {
                            Ok(con) => con.Hash,
                            Err(e) => {
                                eprintln!("Error al subir los tensores al IPFS: {}", e);
                                return;
                            }
                        };

                        let method = npc_clone
                            .npc_publication_contrato
                            .method::<_, H256>(
                                "registerPublication",
                                RegisterPub {
                                    _tensors: format!("ipfs://{}", tensores),
                                    _locale: ISO_CODES.get(locale).unwrap().to_string(),
                                    _collection: U256::from(coleccion_id),  
                                    _profileId: U256::from(perfil_id),      
                                    _pubId: U256::from(publicacion_id + 1), 
                                    _pageNumber: pagina,                    
                                    _lensType: eleccion.as_u8(),            
                                    _boudica: false,
                                },
                            );

    
                        match method {
                            Ok(call) => {
                                let FunctionCall { tx, .. } = call;
    
                                if let Some(tx_request) = tx.as_eip1559_ref() {
                                    let gas_price = U256::from(500_000_000_000u64);
                                    let max_priority_fee = U256::from(25_000_000_000u64);
                                    let gas_limit = U256::from(300_000);
    
                                    let cliente = npc_clone.npc_publication_contrato.client().clone();
                                    // let nonce = cliente
                                    //     .clone()
                                    //     .get_transaction_count(
                                    //         npc_clone.npc.billetera.parse::<Address>().unwrap(),
                                    //         None,
                                    //     )
                                    //     .await
                                    //     .map_err(|e| Box::new(CustomError::new(&e.to_string())) as Box<dyn Error + Send + Sync>)
                                    //     .expect("Error al recuperar el nonce");
    
                                    let req = Eip1559TransactionRequest {
                                        from: Some(npc_clone.npc.billetera.parse::<Address>().unwrap()),
                                        to: Some(NameOrAddress::Address(
                                            NPC_PUBLICATION.parse::<Address>().unwrap(),
                                        )),
                                        gas: Some(gas_limit),
                                        value: tx_request.value,
                                        data: tx_request.data.clone(),
                                        max_priority_fee_per_gas: Some(max_priority_fee),
                                        max_fee_per_gas: Some(gas_price + max_priority_fee),
                                        // nonce: Some(nonce),
                                        chain_id: Some(Chain::Polygon.into()),
                                        ..Default::default()
                                    };

                                        let pending_tx = match cliente.send_transaction(req, None).await {
                                            Ok(tx) => tx,
                                            Err(e) => {
                                                println!("Error al enviar la transacción: {:?}", e);
                                                return;
                                            }
                                        };
                            
                                    let tx_hash = match pending_tx.confirmations(1).await {
                                        Ok(hash) => hash,
                                        Err(e) => {
                                            println!("Error con la transacción: {:?}", e);
                                            return;
                                        }
                                    };
                                    
    
                                    println!("Transacción enviada con hash: {:?}", tx_hash);
                                }
                            }
                            Err(e) => {
                                eprintln!("Error al registrar la publicación: {}", e);
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
            } else {
                eprintln!("Error al parsear los datos JSON");
                return;
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
                            

}



