use std::error::Error;

use dotenv::var;
use rand::{thread_rng, Rng};
use reqwest::Client;
use serde_json::{json, Value};

use crate::bib::types::Coleccion;

pub async fn handle_collections(address: &str) -> Result<Coleccion, Box<dyn Error + Send + Sync>> {
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
        .post( format!(" https://gateway-arbitrum.network.thegraph.com/api/{}/subgraphs/id/8JRara6TGvHV6gKHr5rqeMUsjpAmxe6QHVv8vc23g2KY", graph_key)
           )
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
                titulo: String::from(""),
                descripcion: String::from(""),
                coleccion_id: String::from(""),
            };

            if colecciones.len() > 0 {
                let elegido = &colecciones[thread_rng().gen_range(0..colecciones.len())];

                coleccion = Coleccion {
                    imagen: elegido["metadata"]["image"].to_string(),
                    titulo: elegido["metadata"]["title"].to_string(),
                    descripcion: elegido["metadata"]["description"].to_string(),
                    coleccion_id: elegido["collectionId"].to_string(),
                };
            }

            Ok(coleccion)
        }
        Err(err) => Err(Box::new(err)),
    }
}
