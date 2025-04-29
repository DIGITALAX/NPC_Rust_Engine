use crate::{
    bib::{
        constants::{LENS_CHAIN_ID, SPECTATOR_REWARDS},
        contracts::initialize_contracts,
        graph::{calculate_amount, handle_collections},
        ipfs::{subir_ipfs_imagen, upload_ipfs, upload_lens_storage},
        lens::{
            find_comment, get_mentions, handle_tokens, make_comment, make_like, make_mirror,
            make_publication, make_quote,
        },
        types::{
            Contenido, Coordenada, Estado, GameTimer, Imagen, LensType, Mapa, Movimiento,
            NPCAleatorio, Publicacion, Silla, Sprite, Talla,
        },
        utils::{between, format_instructions, obtener_lens, obtener_pagina},
        venice::{
            call_chat_completion, call_comment_completion, call_gen_image, call_mention,
            call_prompt, call_publication_completion, call_spectate,
        },
    },
    TokensAlmacenados,
};
use ethers::{
    contract::FunctionCall,
    middleware::{Middleware, SignerMiddleware},
    providers::{Http, Provider},
    signers::LocalWallet,
    types::{Address, Eip1559TransactionRequest, NameOrAddress, H160, H256, U256},
};
use pathfinding::prelude::astar;
use rand::{thread_rng, Rng};
use serde_json::to_string;
use std::{
    error::Error,
    io,
    marker::{Send, Sync},
    str::FromStr,
    sync::{Arc, Mutex},
};
use tokio::runtime::Handle;
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
        manija: Handle,
    ) -> Option<Self> {
        let contratos = initialize_contracts(&sprite.etiqueta.to_string());

        match contratos {
            Some((autograph_catalog_contrato, spectator_rewards_contrato)) => Some(NPCAleatorio {
                reloj_juego,
                reloj_au: 0,
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
                spectator_rewards_contrato,
                autograph_catalog_contrato,
                escena,
                tokens: None,
                registro_paginas: vec![],
                registro_colecciones: vec![],
                registro_tipos: vec![],
                ultima_mencion: String::from(""),
                manija,
            }),
            None => {
                eprintln!(
                    "Failed to initialize contracts for agent with sprite: {}",
                    sprite.etiqueta.to_string()
                );
                None
            }
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

        if self.reloj_au < 604_800_000 {
            self.reloj_au += delta_time;
        }

        if self.reloj_au >= 604_800_000 {
            self.add_au_agent();
            self.reloj_au = 0;
        }

        if self.ultimo_tiempo_comprobacion >= self.npc.publicacion_reloj {
            self.ultimo_tiempo_comprobacion = 0;
            self.comprobar_actividad();
            self.agent_spectate();
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
        let sillas_disponibles: Vec<&Silla> = self
            .sillas
            .iter()
            .filter(|silla| {
                !self
                    .sillas_ocupadas
                    .lock()
                    .unwrap()
                    .iter()
                    .any(|silla_ocupada| silla_ocupada.etiqueta == silla.etiqueta)
            })
            .collect();

        if !sillas_disponibles.is_empty() {
            let silla_aleatoria =
                sillas_disponibles[thread_rng().gen_range(0..sillas_disponibles.len())];

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
                nearest =
                    self.encontrar_camino_cercano(silla_x as i32, self.mundo.altura as i32 - 1);
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

    fn agent_spectate(&self) {
        let npc_clone = Arc::new(self.clone());

        self.manija.spawn(async move {
            let chosen_agent =
                &npc_clone.npc.amigos[thread_rng().gen_range(0..npc_clone.npc.amigos.len())];

            match call_spectate(
                &chosen_agent,
                &format_instructions(&npc_clone.npc.prompt),
                &npc_clone.npc.prompt.model,
            )
            .await
            {
                Ok(data) => match upload_ipfs(data).await {
                    Ok(ipfs) => {
                        let method = npc_clone
                            .spectator_rewards_contrato
                            .method::<(String, Address), H256>(
                                "spectate",
                                (
                                    format!("ipfs://{}", ipfs.Hash),
                                    H160::from_str(&chosen_agent).unwrap(),
                                ),
                            );

                        match method {
                            Ok(call) => {
                                let FunctionCall { tx, .. } = call;

                                if let Some(tx_request) = tx.as_eip1559_ref() {
                                    let gas_price = U256::from(500_000_000_000u64);
                                    let max_priority_fee = U256::from(25_000_000_000u64);
                                    let gas_limit = U256::from(300_000);

                                    let client =
                                        npc_clone.spectator_rewards_contrato.client().clone();
                                    let chain_id = *LENS_CHAIN_ID;
                                    let req = Eip1559TransactionRequest {
                                        from: Some(
                                            npc_clone.npc.billetera.parse::<Address>().unwrap(),
                                        ),
                                        to: Some(NameOrAddress::Address(
                                            SPECTATOR_REWARDS.parse::<Address>().unwrap(),
                                        )),
                                        gas: Some(gas_limit),
                                        value: tx_request.value,
                                        data: tx_request.data.clone(),
                                        max_priority_fee_per_gas: Some(max_priority_fee),
                                        max_fee_per_gas: Some(gas_price + max_priority_fee),
                                        chain_id: Some(chain_id.into()),
                                        ..Default::default()
                                    };

                                    match client.send_transaction(req, None).await {
                                        Ok(tx) => {
                                            match tx.confirmations(1).await {
                                                Ok(hash) => {
                                                    println!(
                                                        "Spectate {} TX Hash: {:?} on Agent {}",
                                                        npc_clone.npc.id, hash, chosen_agent
                                                    );
                                                }
                                                Err(e) => {
                                                    eprintln!(
                                                        "Error with transaction confirmation: {:?}",
                                                        e
                                                    );
                                                }
                                            };
                                        }
                                        Err(e) => {
                                            eprintln!(
                                                "Error sending the transaction for spectate: {:?}",
                                                e
                                            );
                                        }
                                    };
                                } else {
                                    eprintln!("Error in sending Transaction");
                                }
                            }

                            Err(err) => {
                                eprintln!("Error in create method for spectate: {:?}", err);
                            }
                        }
                    }
                    Err(err) => {
                        eprintln!("Error with IPFS upload for spectate: {}", err)
                    }
                },
                Err(err) => println!("Error with calling venice spectate {:?}", err),
            }
        });
    }

    fn comprobar_actividad(&self) {
        let mut npc_clone = Arc::new(self.clone());
        let etiqueta = self.npc.etiqueta.clone();
        let account_address = self.npc.account_address.clone();
        let tokens = self.tokens.clone();

        self.manija.spawn(async move {
            match handle_tokens(&etiqueta, &account_address, tokens).await {
                Ok(tokens) => {
                    Arc::get_mut(&mut npc_clone)
                        .unwrap()
                        .actualizar_tokens(tokens);

                    Arc::get_mut(&mut npc_clone)
                        .unwrap()
                        .comprobar_menciones()
                        .await;
                    Arc::get_mut(&mut npc_clone)
                        .unwrap()
                        .hacer_publicacion()
                        .await;
                }
                Err(err) => println!("Error with tokens {:?}", err),
            }
        });
    }

    fn add_au_agent(&self) {
        let npc_clone = Arc::new(self.clone());

        self.manija.spawn(async move {
            let amount = calculate_amount(npc_clone.npc.billetera.clone()).await;

            if amount > U256::from(0) {
                let method = npc_clone
                    .spectator_rewards_contrato
                    .method::<U256, H256>("addAgentAU", amount);

                match method {
                    Ok(call) => {
                        let FunctionCall { tx, .. } = call;

                        if let Some(tx_request) = tx.as_eip1559_ref() {
                            let gas_price = U256::from(500_000_000_000u64);
                            let max_priority_fee = U256::from(25_000_000_000u64);
                            let gas_limit = U256::from(300_000);

                            let client = npc_clone.spectator_rewards_contrato.client().clone();
                            let chain_id = *LENS_CHAIN_ID;
                            let req = Eip1559TransactionRequest {
                                from: Some(npc_clone.npc.billetera.parse::<Address>().unwrap()),
                                to: Some(NameOrAddress::Address(
                                    SPECTATOR_REWARDS.parse::<Address>().unwrap(),
                                )),
                                gas: Some(gas_limit),
                                value: tx_request.value,
                                data: tx_request.data.clone(),
                                max_priority_fee_per_gas: Some(max_priority_fee),
                                max_fee_per_gas: Some(gas_price + max_priority_fee),
                                chain_id: Some(chain_id.into()),
                                ..Default::default()
                            };

                            match client.send_transaction(req, None).await {
                                Ok(tx) => {
                                    match tx.confirmations(1).await {
                                        Ok(hash) => {
                                            println!(
                                                "Agent {} TX Hash: {:?}",
                                                npc_clone.npc.id, hash
                                            );

                                            match npc_clone.agent_pay().await {
                                                Ok(()) => {
                                                    println!(
                                                        "Agent AU distributed for {}",
                                                        npc_clone.npc.etiqueta
                                                    );
                                                }
                                                Err(err) => {
                                                    println!("Error with tokens {:?}", err);
                                                }
                                            }
                                        }
                                        Err(e) => {
                                            eprintln!(
                                                "Error with transaction confirmation: {:?}",
                                                e
                                            );
                                        }
                                    };
                                }
                                Err(e) => {
                                    eprintln!(
                                        "Error sending the transaction for addAgentAU: {:?}",
                                        e
                                    );
                                }
                            };
                        } else {
                            eprintln!("Error in sending Transaction");
                        }
                    }

                    Err(err) => {
                        eprintln!("Error in create method for payRent: {:?}", err);
                    }
                }
            } else {
                println!("No AU calculated");
            }
        });
    }

    async fn agent_pay(&self) -> Result<(), Box<dyn Error + Send + Sync>> {
        let method = self
            .spectator_rewards_contrato
            .method::<(), H256>("agentPayAU", ());

        match method {
            Ok(call) => {
                let FunctionCall { tx, .. } = call;

                if let Some(tx_request) = tx.as_eip1559_ref() {
                    let gas_price = U256::from(500_000_000_000u64);
                    let max_priority_fee = U256::from(25_000_000_000u64);
                    let gas_limit = U256::from(300_000);

                    let client = self.spectator_rewards_contrato.client().clone();
                    let chain_id = *LENS_CHAIN_ID;
                    let req = Eip1559TransactionRequest {
                        from: Some(self.npc.billetera.parse::<Address>().unwrap()),
                        to: Some(NameOrAddress::Address(
                            SPECTATOR_REWARDS.parse::<Address>().unwrap(),
                        )),
                        gas: Some(gas_limit),
                        value: tx_request.value,
                        data: tx_request.data.clone(),
                        max_priority_fee_per_gas: Some(max_priority_fee),
                        max_fee_per_gas: Some(gas_price + max_priority_fee),
                        chain_id: Some(chain_id.into()),
                        ..Default::default()
                    };

                    let pending_tx = match client.send_transaction(req, None).await {
                        Ok(tx) => tx,
                        Err(e) => {
                            eprintln!("Error sending the transaction for agentPayAU: {:?}", e);
                            Err(Box::new(e))?
                        }
                    };

                    let tx_hash = match pending_tx.confirmations(1).await {
                        Ok(hash) => hash,
                        Err(e) => {
                            eprintln!("Error with transaction confirmation: {:?}", e);
                            Err(Box::new(e))?
                        }
                    };

                    println!("Agent {} TX Hash: {:?}", self.npc.id, tx_hash);

                    Ok(())
                } else {
                    eprintln!("Error in sending Transaction");
                    Err(Box::new(io::Error::new(
                        io::ErrorKind::Other,
                        format!("Error in sending Transaction"),
                    )))
                }
            }

            Err(err) => {
                eprintln!("Error in create method for agent_pay: {:?}", err);
                Err(Box::new(io::Error::new(
                    io::ErrorKind::Other,
                    format!("Error in create method for agent_pay {:?}", err),
                )))
            }
        }
    }

    async fn comprobar_menciones(&mut self) {
        let access_tokens = self.tokens.clone().unwrap().tokens.access_token;

        match get_mentions(&access_tokens, &self.ultima_mencion).await {
            Ok(menciones) => {
                if let Some(ultima) = menciones.last() {
                    self.ultima_mencion = ultima.id.clone();
                    for mencion in menciones {
                        match call_mention(
                            &mencion.content,
                            &format_instructions(&self.npc.prompt),
                            &self.npc.prompt.model,
                        )
                        .await
                        {
                            Ok(content) => {
                                match formatear_pub(
                                    Some(content),
                                    None,
                                    LensType::Comment,
                                    &mencion.post_id,
                                    &self.escena,
                                    &access_tokens,
                                    &self.npc.etiqueta,
                                )
                                .await
                                {
                                    Ok(()) => {
                                        println!("Mention response sent");
                                    }
                                    Err(err) => {
                                        eprintln!("Error in sending mention response {}", err);
                                    }
                                }
                            }
                            Err(err) => eprintln!("Error in venice mention {}", err),
                        }
                    }
                } else {
                    eprintln!("No mentione");
                }
            }
            Err(err) => {
                eprintln!("Error in mentions {}", err);
            }
        }
    }

    async fn hacer_publicacion(&mut self) {
        let access_tokens = self.tokens.clone().unwrap().tokens.access_token;
        let lens_tipo = obtener_lens(self.registro_tipos.clone());
        self.registro_tipos.push(lens_tipo);

        let account_address = &self.npc.amigos[thread_rng().gen_range(0..self.npc.amigos.len())];

        if lens_tipo == LensType::Autograph || lens_tipo == LensType::Catalog {
            let mut imagen = String::from("");
            let mut descripcion = String::from("");

            if lens_tipo == LensType::Autograph {
                match handle_collections(&self.npc.billetera, self.registro_colecciones.clone())
                    .await
                {
                    Ok(coleccion) => {
                        imagen = coleccion.imagen;

                        descripcion = coleccion.descripcion;

                        self.registro_colecciones
                            .push(U256::from_str(&coleccion.coleccion_id).unwrap());
                    }
                    Err(err) => {
                        eprintln!("Un error de obtener la colección {}", err);
                    }
                }
            } else {
                let pagina = obtener_pagina(self.registro_paginas.clone());
                self.registro_paginas.push(U256::from(pagina));

                let metodo = self
                    .autograph_catalog_contrato
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
                                imagen = uri;
                                descripcion = String::from("The Autograph Quarterly.");
                            }
                            Err(e) => {
                                eprintln!("Error al obtener la página del catálogo: {}", e);
                                return;
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("Un error de ABI {}", e);
                        return;
                    }
                }
            }

            match call_chat_completion(
                &descripcion,
                &format_instructions(&self.npc.prompt),
                &self.npc.prompt.model,
            )
            .await
            {
                Ok(content) => {
                    match formatear_pub(
                        Some(content),
                        Some(imagen),
                        lens_tipo,
                        "",
                        &self.escena,
                        &access_tokens,
                        &self.npc.etiqueta,
                    )
                    .await
                    {
                        Ok(()) => {
                            println!("Publication sent for autograph/catalog");
                        }
                        Err(err) => {
                            eprintln!("Error in sending for autograph/catalog {}", err);
                        }
                    }
                }
                Err(err) => {
                    eprintln!("Error in sending for autograph/catalog {}", err)
                }
            }
        } else if lens_tipo == LensType::Comment
            || lens_tipo == LensType::Quote
            || lens_tipo == LensType::Mirror
        {
            let (contenido, comentario_id) =
                match find_comment(&access_tokens, &account_address).await {
                    Ok(result) => result,
                    Err(e) => {
                        eprintln!("Error al encontrar el comentario: {}", e);
                        return;
                    }
                };

            if lens_tipo != LensType::Mirror {
                match call_comment_completion(
                    &contenido,
                    &format_instructions(&self.npc.prompt),
                    &self.npc.prompt.model,
                )
                .await
                {
                    Ok((content, image)) => {
                        let mut imagen: Option<String> = None;

                        if image {
                            imagen = match call_prompt(&content, &self.npc.prompt.model).await {
                                Ok((prompt, image_model)) => {
                                    match call_gen_image(&prompt, &image_model).await {
                                        Ok(image) => Some(image),
                                        Err(_) => None,
                                    }
                                }
                                Err(_) => None,
                            }
                        }

                        match formatear_pub(
                            Some(content),
                            imagen,
                            lens_tipo,
                            &comentario_id,
                            &self.escena,
                            &access_tokens,
                            &self.npc.etiqueta,
                        )
                        .await
                        {
                            Ok(()) => {
                                println!("Publication sent for comment/quote");
                            }
                            Err(err) => {
                                eprintln!("Error in sending for comment/quote {}", err);
                            }
                        }
                    }
                    Err(err) => {
                        eprintln!("Error in sending for comment/quote {}", err);
                    }
                }
            } else {
                match formatear_pub(
                    None,
                    None,
                    lens_tipo,
                    &comentario_id,
                    &self.escena,
                    &access_tokens,
                    &self.npc.etiqueta,
                )
                .await
                {
                    Ok(()) => {
                        println!("Publication sent for mirror");
                    }
                    Err(err) => {
                        eprintln!("Error in sending for mirror {}", err);
                    }
                }
            }
        } else {
            match call_publication_completion(
                &format_instructions(&self.npc.prompt),
                &self.npc.prompt.model,
            )
            .await
            {
                Ok(content) => {
                    let imagen = match call_prompt(&content, &self.npc.prompt.model).await {
                        Ok((prompt, image_model)) => {
                            match call_gen_image(&prompt, &image_model).await {
                                Ok(image) => Some(image),
                                Err(_) => None,
                            }
                        }
                        Err(_) => None,
                    };

                    match formatear_pub(
                        Some(content),
                        imagen,
                        lens_tipo,
                        "",
                        &self.escena,
                        &access_tokens,
                        &self.npc.etiqueta,
                    )
                    .await
                    {
                        Ok(()) => {
                            println!("Publication sent for publication");
                        }
                        Err(err) => {
                            eprintln!("Error in sending for publication {}", err);
                        }
                    }
                }
                Err(err) => {
                    eprintln!("Error in sending for publication {}", err);
                }
            }
        }
    }

    pub fn actualizar_tokens(&mut self, nuevos_tokens: TokensAlmacenados) {
        self.tokens = Some(nuevos_tokens);
    }
}

async fn formatear_pub(
    contenido: Option<String>,
    imagen: Option<String>,
    lens_tipo: LensType,
    comentario_id: &str,
    escena: &str,
    access_token: &str,
    etiqueta: &str,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let mut contenido_subido: Option<String> = None;
    if contenido.is_some() {
        let mut imagen_url: Option<Imagen> = None;
        let mut enfoque = "TEXT_ONLY".to_string();
        let mut schema = "https://json-schemas.lens.dev/posts/text-only/3.0.0.json".to_string();

        if let Some(base64_imagen) = imagen {
            if base64_imagen.contains("ipfs://") {
                let opcion = Imagen {
                    tipo: "image/png".to_string(),
                    item: String::from(base64_imagen),
                };
                imagen_url = Some(opcion);
            } else {
                match subir_ipfs_imagen(&base64_imagen).await {
                    Ok(cid) => {
                        let opcion = Imagen {
                            tipo: "image/png".to_string(),
                            item: format!("ipfs://{}", cid.Hash),
                        };
                        imagen_url = Some(opcion);
                    }
                    Err(e) => {
                        println!("Error en formatear la publicacion {}", e);
                    }
                }
            }
        }

        if let Some(_) = imagen_url.as_ref() {
            enfoque = String::from("IMAGE");
            schema = "https://json-schemas.lens.dev/posts/image/3.0.0.json".to_string();
        }

        let tags = vec!["npcStudio".to_string(), escena.to_string().replace(" ", "")];

        let content = contenido.unwrap();

        let publicacion = Publicacion {
            schema,
            lens: Contenido {
                mainContentFocus: enfoque,
                title: content.chars().take(20).collect(),
                content,
                id: Uuid::new_v4().to_string(),
                locale: "en".to_string(),
                tags,
                image: imagen_url,
            },
        };

        let publicacion_json = to_string(&publicacion)?;

        contenido_subido = match upload_lens_storage(publicacion_json).await {
            Ok(con) => Some(con),
            Err(e) => {
                eprintln!("Error uploading content to Lens Storage: {}", e);
                return Err(Box::new(io::Error::new(
                    io::ErrorKind::Other,
                    format!("Error uploading content to Lens Storage: {}", e),
                )));
            }
        };
    }

    if lens_tipo == LensType::Mirror
        || lens_tipo == LensType::Quote
        || lens_tipo == LensType::Comment
    {
        match make_like(access_token, comentario_id).await {
            Ok(_) => {}
            Err(e) => {
                eprintln!("Error al gustar la publicacion al IPFS: {}", e);
            }
        }
    }

    match enviar_mensaje(
        contenido_subido,
        lens_tipo,
        comentario_id,
        access_token,
        etiqueta,
    )
    .await
    {
        Ok(resultado) => {
            return {
                println!("Message from result: {}", resultado);

                Ok(())
            }
        }
        Err(e) => {
            eprintln!("Error al enviar el mensaje: {:?}", e);
            return Err(Box::new(io::Error::new(
                io::ErrorKind::Other,
                format!("Error al enviar el mensaje {:?}", e),
            )));
        }
    }
}

async fn enviar_mensaje(
    contenido: Option<String>,
    lens_tipo: LensType,
    comentario_id: &str,
    access_tokens: &str,
    etiqueta: &str,
) -> Result<String, Box<dyn Error + Send + Sync>> {
    if lens_tipo == LensType::Comment || lens_tipo == LensType::Quote {
        if lens_tipo == LensType::Quote {
            make_quote(
                &contenido.unwrap(),
                &etiqueta,
                &access_tokens,
                &comentario_id,
            )
            .await
        } else {
            make_comment(
                &contenido.unwrap(),
                &etiqueta,
                &access_tokens,
                &comentario_id,
            )
            .await
        }
    } else if lens_tipo == LensType::Mirror {
        make_mirror(&access_tokens, &comentario_id).await
    } else {
        make_publication(&contenido.unwrap(), &etiqueta, &access_tokens).await
    }
}
