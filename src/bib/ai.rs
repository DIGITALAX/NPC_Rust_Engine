use crate::bib::{
    constants::INFURA_GATEWAY,
    types::{OpenAIRespuesta, OpenAIUso},
};
use dotenv::from_filename;
use rand::{thread_rng, Rng};
use reqwest::Client;
use serde_json::{json, Value};
use std::{env::var, error::Error, io};

pub async fn call_chat_completion_openai(
    prompt: &str,
    imagen: Option<String>,
) -> Result<OpenAIRespuesta, Box<dyn Error + Send + Sync>> {
    from_filename(".env").ok();
    let open_ai_key: String = var("OPEN_AI_SECRET").expect("OPEN_AI_SECRET not configured in .env");
    let max_completion_tokens = [100, 200, 350][thread_rng().gen_range(0..3)];
    let mut messages = vec![];

    if imagen.is_some() {
        let imagen = imagen.unwrap();
        messages.push(json!({
            "role": "user",
            "content": [
                {
                  "type": "text",
                  "text": prompt
                },
                {
                  "type": "image_url",
                  "image_url": format!("{}/ipfs/{}", INFURA_GATEWAY, imagen.strip_prefix("ipfs://").unwrap_or(&imagen))
                }
              ]
        }));
    } else {
        messages.push(json!({
            "role": "user",
            "content": prompt
        }));
    }

    let client = Client::new();
    let request_body = json!({
        "model": "gpt-4o-mini",
        "messages": messages,
        "max_completion_tokens": max_completion_tokens,
        "n": 1,
    });

    let response = client
        .post("https://api.openai.com/v1/chat/completions")
        .header("Authorization", format!("Bearer {}", open_ai_key))
        .json(&request_body)
        .send()
        .await;

    let response = match response {
        Ok(resp) => resp,
        Err(e) => {
            eprintln!("Error sending request to OpenAI API: {}", e);
            return Err(e.into());
        }
    };
    if response.status() == 200 {
        let response_json: Value = response.json().await?;

        Ok(OpenAIRespuesta {
            complecion: response_json["choices"][0]["message"]["content"]
                .as_str()
                .unwrap_or("")
                .to_string(),
            modelo: response_json["model"].as_str().unwrap_or("").to_string(),
            uso: response_json["usage"]
                .as_object()
                .map(|obj| {
                    serde_json::from_value::<OpenAIUso>(serde_json::Value::Object(obj.clone()))
                        .unwrap_or(OpenAIUso {
                            prompt_tokens: 0,
                            completion_tokens: 0,
                            total_tokens: 0,
                        })
                })
                .unwrap_or(OpenAIUso {
                    prompt_tokens: 0,
                    completion_tokens: 0,
                    total_tokens: 0,
                }),
        })
    } else {
        return Err(Box::new(io::Error::new(
            io::ErrorKind::Other,
            format!("Error in obtaining OpenAI prompt {:?}", response.status()),
        )));
    }
}
