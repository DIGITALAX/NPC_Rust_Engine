use std::{collections::HashMap, error::Error};

use dotenv::var;
use ethers::types::U256;
use reqwest::Client;
use serde_json::{json, Value};

use crate::bib::{
    constants::GRAPH_URI,
    lens::handle_lens_account,
    types::{Coleccion, Escena, HalfSprite, MessageExample, Prompt, Sprite, Text},
    utils::{fetch_metadata, obtener_coleccion},
};

use super::constants::LISTA_ESCENA;

pub async fn handle_collections(
    address: &str,
    registro_colecciones: Vec<U256>,
) -> Result<Coleccion, Box<dyn Error + Send + Sync>> {
    let client = Client::new();

    let query = json!({
        "query": r#"
        query(id: String!) {
            agentAssigneds(where: {id: $id}) {
                collections {
                   collectionId
                   metadata {
                        image
                        description
                        title
                   }
                }
            }
        }
        "#,
        "variables": {
            "request": {
                "where": {
                    "id": [address]
                }
            }
        }
    });

    let graph_key: String = var("GRAPH_KEY").expect("GRAPH_KEY not configured in .env");
    let res = client
        .post(format!(
            "{}{}/subgraphs/id/8JRara6TGvHV6gKHr5rqeMUsjpAmxe6QHVv8vc23g2KY",
            GRAPH_URI, graph_key
        ))
        .json(&query)
        .send()
        .await;

    match res {
        Ok(response) => {
            let parsed: Value = response.json().await?;
            let empty_vec = vec![];
            let agent_assigned = &parsed["data"]["agentAssigneds"]
                .as_array()
                .unwrap_or(&empty_vec)[0];

            let colecciones = agent_assigned["collections"]
                .as_array()
                .unwrap_or(&empty_vec);
            let mut coleccion = Coleccion {
                imagen: String::from(""),
                descripcion: String::from(""),
                coleccion_id: String::from(""),
            };

            if colecciones.len() > 0 {
                let elegido = obtener_coleccion(registro_colecciones, colecciones.len() as u8);

                coleccion = Coleccion {
                    imagen: colecciones[elegido as usize]["metadata"]["image"].to_string(),
                    descripcion: colecciones[elegido as usize]["metadata"]["description"]
                        .to_string(),
                    coleccion_id: colecciones[elegido as usize]["collectionId"].to_string(),
                };
            }

            Ok(coleccion)
        }
        Err(err) => Err(Box::new(err)),
    }
}

pub async fn handle_agents() -> Result<HashMap<String, HalfSprite>, Box<dyn Error + Send + Sync>> {
    let client = Client::new();

    let query = json!({
        "query": r#"
        query {
            agentCreateds(first: 100, where: { studio: true }) {
                wallets
                SkyhuntersAgentManager_id
                creator
                uri
                metadata {
                    title
                    bio
                    lore
                    adjectives
                    style
                    knowledge
                    messageExamples
                    model
                    cover
                    customInstructions
                }
            }
        }
        "#,
    });

    let graph_key: String = var("GRAPH_KEY").expect("GRAPH_KEY not configured in .env");
    let res = client
        .post(format!(
            "{}{}/subgraphs/id/8JRara6TGvHV6gKHr5rqeMUsjpAmxe6QHVv8vc23g2KY",
            GRAPH_URI, graph_key
        ))
        .json(&query)
        .send()
        .await;

    match res {
        Ok(response) => {
            let parsed: Value = response.json().await?;
            let empty_vec = vec![];
            let agent_createds = parsed["data"]["agentCreateds"]
                .as_array()
                .unwrap_or(&empty_vec);

            let mut agents_snapshot: HashMap<String, HalfSprite> = HashMap::new();

            for agent_created in agent_createds {
                let new_id: u32 = agent_created["SkyhuntersAgentManager_id"]
                    .as_str()
                    .unwrap_or("0")
                    .parse()
                    .map_err(|_| "Failed to parse ID")?;

                let billetera = agent_created["wallets"]
                    .as_array()
                    .unwrap_or(&vec![])
                    .get(0)
                    .and_then(|w| w.as_str())
                    .unwrap_or("")
                    .to_string();
                let account_address = handle_lens_account(&billetera, false)
                    .await
                    .unwrap_or_default();

                let metadata = agent_created["metadata"].clone();
                let is_metadata_empty = metadata.is_null()
                    || metadata.as_object().map(|o| o.is_empty()).unwrap_or(false);

                let metadata_filled = if is_metadata_empty {
                    if let Some(uri) = agent_created["uri"].as_str() {
                        fetch_metadata(uri).await.unwrap_or(json!({}))
                    } else {
                        json!({})
                    }
                } else {
                    metadata
                };

                let sprite = HalfSprite {
                    id: new_id,
                    prompt: Prompt {
                        bio: metadata_filled["bio"].as_str().unwrap_or("").to_string(),
                        lore: metadata_filled["lore"].as_str().unwrap_or("").to_string(),
                        adjectives: metadata_filled["adjectives"]
                            .as_str()
                            .unwrap_or("")
                            .to_string(),
                        style: metadata_filled["style"].as_str().unwrap_or("").to_string(),
                        knowledge: metadata_filled["knowledge"]
                            .as_str()
                            .unwrap_or("")
                            .to_string(),
                        model: metadata_filled["model"]
                            .as_str()
                            .unwrap_or("dolphin-2.9.2-qwen2-72b")
                            .to_string(),
                        custom_instructions: metadata_filled["customInstructions"]
                            .as_str()
                            .unwrap_or("")
                            .to_string(),
                        message_examples: metadata_filled["message_examples"]
                            .as_array()
                            .unwrap_or(&vec![])
                            .iter()
                            .map(|v| {
                                v.as_array()
                                    .unwrap_or(&vec![])
                                    .iter()
                                    .map(|con| {
                                        let parsed_con: MessageExample =
                                            serde_json::from_str(con.as_str().unwrap_or("{}"))
                                                .unwrap_or(MessageExample {
                                                    user: "".to_string(),
                                                    content: Text {
                                                        text: "".to_string(),
                                                    },
                                                });

                                        parsed_con
                                    })
                                    .collect::<Vec<MessageExample>>()
                            })
                            .collect::<Vec<Vec<MessageExample>>>(),
                        cover: metadata_filled["cover"].as_str().unwrap_or("").to_string(),
                    },
                    billetera,
                    account_address,
                };

                agents_snapshot.insert(
                    metadata_filled["title"].as_str().unwrap_or("").to_string(),
                    sprite,
                );
            }
            Ok(agents_snapshot)
        }
        Err(err) => Err(Box::new(err)),
    }
}

pub async fn handle_escenas() -> Result<Vec<Escena>, Box<dyn Error + Send + Sync>> {
    let agents = handle_agents().await?;

    Ok(LISTA_ESCENA
        .iter()
        .map(|escena| Escena {
            clave: escena.clave.clone(),
            mundo: escena.mundo.clone(),
            imagen: escena.imagen.clone(),
            prohibido: escena.prohibido.clone(),
            profundidad: escena.profundidad.clone(),
            sillas: escena.sillas.clone(),
            fondo: escena.fondo.clone(),
            objetos: escena.objetos.clone(),
            interactivos: escena.interactivos.clone(),
            sprites: escena
                .sprites
                .iter()
                .filter_map(|sprite| {
                    agents.get(&sprite.etiqueta).map(|half| Sprite {
                        id: half.id,
                        etiqueta: sprite.etiqueta.clone(),
                        uri: sprite.uri.clone(),
                        billetera: half.billetera.clone(),
                        x: sprite.x,
                        y: sprite.y,
                        altura: sprite.altura,
                        anchura: sprite.anchura,
                        anchura_borde: sprite.anchura_borde,
                        altura_borde: sprite.altura_borde,
                        margen: sprite.margen,
                        tapa: sprite.tapa.clone(),
                        marco_inicio: sprite.marco_inicio,
                        marco_final: sprite.marco_final,
                        movimientos_max: sprite.movimientos_max,
                        escala: sprite.escala.clone(),
                        publicacion_reloj: sprite.publicacion_reloj,
                        amigos: sprite.amigos.clone(),
                        account_address: half.account_address.clone(),
                        prompt: half.prompt.clone(),
                    })
                })
                .collect(),
        })
        .collect())
}
