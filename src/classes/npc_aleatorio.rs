use crate::{bib::{ai::call_chat_completion_openai, graph::handle_collections, lens, types::{
    Comment, Contenido, Coordenada, CustomError, Estado, GameTimer, Imagen, LensType, Mapa, Mirror, Movimiento, NPCAleatorio, OpenAIRespuesta, OpenAIUso, Pub, Publicacion,  Silla, Sprite, Talla
}, utils::{between, subir_ipfs, subir_ipfs_imagen}}, MetadataAttribute, TokensAlmacenados, CONVERSACION, ISO_CODES, ISO_CODES_PROMPT, LENS_HUB_PROXY};
use abi::{Token, Tokenize};
use ethers::{prelude::*, types::{Address, Bytes, U256}};
use pathfinding::prelude::astar;
use serde_json::{to_string, json};
use rand::{prelude::SliceRandom, thread_rng, Rng};
use tokio::runtime::Handle;
use std::{error::Error, marker::{Send,Sync}, str::FromStr, sync::{Arc, Mutex}};
use uuid::Uuid;
use strum::IntoEnumIterator;

impl NPCAleatorio {
    pub fn new(
        sprite: Sprite,
        sillas_ocupadas: Arc<Mutex<Vec<Silla>>>,
        sillas: Vec<Silla>,
        mundo: Talla,
        reloj_juego: GameTimer,
        mapa: Mapa,
        escena: String,
        manija: Handle) -> Self {
        let (lens_hub_contrato, autograph_data_contrato) =
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
            escena,
            manija,
            tokens: None,
        }
    }

    pub fn conseguir_estado(&self) -> &Vec<Estado> {
        &self.caminos
    }

    pub fn actualizar(&mut self, delta_time: u64) {
        self.reloj_juego.tick(delta_time);
        self.elegir_direccion_aleatoria();
        self.limpiar_caminos();

        if self.ultimo_tiempo_comprobacion < self.npc.publicacion_reloj {
            self.ultimo_tiempo_comprobacion += delta_time;
        }
        

        if self.ultimo_tiempo_comprobacion >= self.npc.publicacion_reloj {
            self.ultimo_tiempo_comprobacion = 0;
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
        let mut npc_clone = Arc::new(self.clone());
       
        self.manija.spawn(async move {

            let tokens = lens::obtener_o_refrescar_tokens(&npc_clone.npc.etiqueta.to_string(), 
            npc_clone.npc.perfil_id
            , npc_clone.tokens.clone()
        
        )
            .await 
            .map_err(|e| Box::new(CustomError::new(&e.to_string())) as Box<dyn Error + Send + Sync  >);

            let mut prompt = String::from("");
            let mut titulo = String::from("");
            let mut descripcion = String::from("");
            let mut imagen: Option<String> = None;
            let mut locale =npc_clone.npc.prompt.idiomas.first().unwrap().to_string();
            let limite_palabra = [100,200,350][thread_rng().gen_range(0..3)] ;
            let etiquetas = [" and at the end include some hashtags. ", " and do not include any hashtags. "][thread_rng().gen_range(0..2)];
            let mut  comentario_perfil = U256::from(0);
            let mut comentario_pub= U256::from(0);
            let mut metadata_uri: String = String::from("");
            let lens_tipo = *LensType::iter().collect::<Vec<_>>().choose(&mut thread_rng()).unwrap();
            let perfil_id = npc_clone.npc.prompt.amigos[thread_rng().gen_range(0..npc_clone.npc.prompt.amigos.len())];
            




match tokens {

    Ok (nuevos_tokens) => {
        Arc::get_mut(&mut npc_clone).unwrap().actualizar_tokens(nuevos_tokens.clone());


       
     

                        let mut haz_pub = false;
                    if lens_tipo == LensType::Autograph {

                        
                        match handle_collections(&npc_clone.npc.billetera).await {

                                Ok(coleccion) => {
                                    imagen =
                                    Some(coleccion.imagen);

                                    titulo =
                                   coleccion.titulo;
descripcion =
coleccion.descripcion;
                                    haz_pub = true;
                                },
                                Err(err) => {
                                    eprintln!("Un error de obtener la colección {}", err);
                                }
        
                        }
                 
                               
                     

                    }  else if lens_tipo == LensType::Catalog {
                        let pagina = thread_rng().gen_range(1..=54);
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
                  

                           if lens_tipo == LensType::Comment || lens_tipo == LensType::Mirror || lens_tipo == LensType::Quote {





                         

                          
let (contenido, perfil, publicacion, metadata) = match lens::coger_comentario(&format!("0x0{:x}", perfil_id)).await {
    Ok(result) => result,
    Err(e) => {
        eprintln!("Error al encontrar el comentario: {}", e);
        return;
    }
};

   if metadata != ""  {
     

           
       let mut rng = thread_rng();
       if let Some(idioma_aleatorio) = npc_clone.npc.prompt.idiomas.choose(&mut rng) {
           locale = idioma_aleatorio.to_string();
       }

        

          metadata_uri = metadata;
           comentario_perfil = perfil;
           comentario_pub = publicacion;
           prompt = format!("{} {} {} {} {} {} {} {} {} {:?} {} {:?} {} {}",CONVERSACION[0], npc_clone.npc.etiqueta.as_str(), CONVERSACION[1], &npc_clone.npc.prompt.tono.join(", "), CONVERSACION[2], &limite_palabra.to_string(), CONVERSACION[3], &contenido,  CONVERSACION[4],  ISO_CODES_PROMPT
           .get(locale.as_str()), CONVERSACION[5], ISO_CODES_PROMPT
           .get(locale.as_str()), &etiquetas, CONVERSACION[6]);

   } else {
       haz_pub = true;
   }





                      

                           } else {
                               haz_pub = true;
                           } 


                         
                            
                        
                        


                    }


                    if haz_pub {

                                                  
                        
                                                   prompt = if lens_tipo ==LensType::Autograph {
                                                    format!("{} {} {} {} {} {} {} {} {:?} {} {:?} {} {}",CONVERSACION[0], npc_clone.npc.etiqueta.as_str(), CONVERSACION[1], &npc_clone.npc.prompt.tono.join(", "), CONVERSACION[2], &limite_palabra.to_string(), format!("extending on the flow, content and ideas of the attached image that has this title: {}, and description {}", titulo, descripcion),  CONVERSACION[4], ISO_CODES_PROMPT
                                                    .get(locale.as_str()), CONVERSACION[5], ISO_CODES_PROMPT
                                                    .get(locale.as_str()), &etiquetas, CONVERSACION[6])
                                                   } else if lens_tipo == LensType::Catalog {
                                                    format!("{} {} {} {} {} {} {} {} {:?} {} {:?} {} {}",CONVERSACION[0], npc_clone.npc.etiqueta.as_str(), CONVERSACION[1], &npc_clone.npc.prompt.tono.join(", "), CONVERSACION[2], &limite_palabra.to_string(), "creating either a short story, commentary or other interesting response about the attached image",  CONVERSACION[4], ISO_CODES_PROMPT
                                                    .get(locale.as_str()), CONVERSACION[5], ISO_CODES_PROMPT
                                                    .get(locale.as_str()), &etiquetas, CONVERSACION[6])
                                                   } else {
                                                    format!("{} {} {} {} {} {} {} {} {} {:?} {} {:?} {} {}",CONVERSACION[0], npc_clone.npc.etiqueta.as_str(), CONVERSACION[1], &npc_clone.npc.prompt.tono.join(", "), CONVERSACION[2], &limite_palabra.to_string(), CONVERSACION[7], &npc_clone.npc.prompt.temas[thread_rng().gen_range(0..npc_clone.npc.prompt.temas.len())],  CONVERSACION[4], ISO_CODES_PROMPT
                                                    .get(locale.as_str()), CONVERSACION[5], ISO_CODES_PROMPT
                                                    .get(locale.as_str()), &etiquetas, CONVERSACION[6])
                                                   };
                        
                                   
                                               }


                                               if lens_tipo == LensType::Mirror {
                                                let _ = npc_clone. formatear_pub(
                                                    metadata_uri,
                                                    OpenAIRespuesta {
                                                        complecion: String::from(""),
                                                        modelo: String::from(""),
                                                        uso: OpenAIUso {
                                                            prompt_tokens: 0,
                                                            completion_tokens: 0,
                                                            total_tokens: 0,
                                                        }
                                                    },
                                                    &locale.clone(),
                                                    None, 
                                                    lens_tipo.clone(),
                                                    comentario_perfil,
                                                    comentario_pub,
                        
                        
                        
                                                   ).await ; 
                                            } else {
                                                match call_chat_completion_openai(&prompt, imagen.clone()).await {
                                                    Ok (respuesta) => {

                                                       

                                                        let _ = npc_clone
    .formatear_pub(
        metadata_uri,
        respuesta.clone(),
        &locale,
        imagen.as_deref(), 
        lens_tipo.clone(),
        comentario_perfil,
        comentario_pub,
    )
    .await;
 
                                                    },
                                                    Err(err) => {
                                                        eprintln!("Error con la generación del mensaje de OpenAI: {:?}", err);
                                                        return;
                                                    }
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
        respuesta: OpenAIRespuesta,
        locale: &str,
        imagen: Option<&str>,
        lens_tipo: LensType,
        comentario_perfil: U256,
        comentario_pub: U256
    ) -> Result<(), Box<dyn Error + Send + Sync  >>{
        let mut imagen_url: Option<Imagen> = None;
        let mut enfoque = "TEXT_ONLY".to_string();
        let mut schema =
            "https://json-schemas.lens.dev/publications/text-only/3.0.0.json".to_string();

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

        let tags =  vec!["npcStudio".to_string(), self.escena.clone()];

        let app_id =  "npcstudio".to_string();
    
        let publicacion = Publicacion {
            schema,
            lens: Contenido {
                mainContentFocus: enfoque,
                title: respuesta.complecion.chars().take(20).collect(),
                content: respuesta.complecion,
                appId: app_id,
                id: Uuid::new_v4().to_string(),
                hideFromFeed: false,
                locale: ISO_CODES.get(locale).unwrap().to_string(),
                tags,
                image: imagen_url,
                attributes:  vec![ 
                    MetadataAttribute {
                        key: "llm_info".to_string(),
                        tipo: "String".to_string(),
                       value: json!({
                            "model": respuesta.modelo,
                            "usage": respuesta.uso
                        }).to_string()
                    }
                ].into(),
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
        match  lens::me_gusta(&format!("0x0{:x}-0x{:02x}", comentario_perfil, comentario_pub), &self.tokens.as_ref().unwrap().tokens.access_token).await {
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
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
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

       match res {
        Ok(result) if result != "RelaySuccess" => {
            let FunctionCall { tx, .. } = method;
    
            if let Some(tx_request) = tx.as_eip1559_ref() {
                let cliente = self.lens_hub_contrato.client().clone();
                let gas_price = U256::from(500_000_000_000u64);
                let max_priority_fee = U256::from(25_000_000_000u64);
                let gas_limit = U256::from(300_000);
                let tx_cost = gas_limit * gas_price + max_priority_fee;
    
                let balance = cliente
                    .clone()
                    .get_balance(
                        self.npc.billetera.parse::<Address>().unwrap(),
                        None,
                    )
                    .await?;
    
                if balance < tx_cost {
                    return Err(Box::new(CustomError::new("Fondos insuficientes para gas")));
                }
    
                let req = Eip1559TransactionRequest {
                    from: Some(self.npc.billetera.parse::<Address>().unwrap()),
                    to: Some(NameOrAddress::Address(
                        LENS_HUB_PROXY.parse::<Address>().unwrap(),
                    )),
                    gas: Some(gas_limit),
                    value: tx_request.value,
                    data: tx_request.data.clone(),
                    max_priority_fee_per_gas: Some(max_priority_fee),
                    max_fee_per_gas: Some(gas_price + max_priority_fee),
                    chain_id: Some(Chain::Polygon.into()),
                    ..Default::default()
                };
                
                let pending_tx = cliente.send_transaction(req, None).await.map_err(|e| {
                    eprintln!("Error al enviar la transacción: {:?}", e);
                    Box::new(CustomError::new("Error al enviar la transacción")) as Box<dyn Error + Send + Sync>
                })?;
        
                let tx_hash = pending_tx.confirmations(1).await.map_err(|e| {
                    eprintln!("Error con la transacción: {:?}", e);
                    Box::new(CustomError::new("Error con la transacción")) as Box<dyn Error + Send + Sync>
                })?;
                
                println!("Transacción del mensaje enviada con hash: {:?}", tx_hash);

                
                Ok(())
            } else {
                Err(Box::new(CustomError::new("Error en Transacción")) as Box<dyn Error + Send + Sync>)
            }
        }
        Ok(other) => {
            eprintln!("Error al enviar el mensaje: {:?}", other);
            Err(Box::new(CustomError::new("Error inesperado al enviar el mensaje")) as Box<dyn Error + Send + Sync>)
        }
        Err(e) => {
            eprintln!("Error al enviar el mensaje: {:?}", e);
            Err(Box::new(CustomError::new("Error al procesar el mensaje")) as Box<dyn Error + Send + Sync>)
        }
    }
    

 
    }

    pub fn actualizar_tokens(&mut self, nuevos_tokens: TokensAlmacenados) {
        self.tokens = Some(nuevos_tokens);
    }
    

                  
}



