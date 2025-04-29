use crate::bib::{
    constants::INFURA_GATEWAY,
    types::{LensType, Prompt},
};
use ethers::types::U256;
use rand::{seq::SliceRandom, thread_rng, Rng};
use regex::Regex;
use reqwest::Client;
use serde_json::{json, to_string, Value};
use std::error::Error;
use strum::IntoEnumIterator;

pub fn between(min: f32, max: f32) -> f32 {
    let mut rng = thread_rng();
    rng.gen_range(min..=max)
}

pub fn obtener_lens(registro_tipos: Vec<LensType>) -> LensType {
    let mut rng = thread_rng();
    let all_types: Vec<LensType> = LensType::iter().collect();

    let last_type = registro_tipos.last().copied();

    let available_types: Vec<LensType> = all_types
        .into_iter()
        .filter(|&t| Some(t) != last_type)
        .collect();

    let selected = *available_types.choose(&mut rng).unwrap();

    selected
}

pub fn obtener_pagina(registro_paginas: Vec<U256>) -> u8 {
    let mut rng = thread_rng();
    let all_pages: Vec<u8> = (1..=54).collect();

    let last_page = registro_paginas.last().copied();

    let available_pages: Vec<u8> = all_pages
        .into_iter()
        .filter(|&p| Some(U256::from(p)) != last_page)
        .collect();

    let selected = *available_pages.choose(&mut rng).unwrap();

    selected
}

pub fn obtener_coleccion(registro_colecciones: Vec<U256>, tamano: u8) -> u8 {
    let mut rng = thread_rng();
    let all_pages: Vec<u8> = (1..=tamano).collect();

    let last_page = registro_colecciones.last().copied();

    let available_pages: Vec<u8> = all_pages
        .into_iter()
        .filter(|&p| Some(U256::from(p)) != last_page)
        .collect();

    let selected = *available_pages.choose(&mut rng).unwrap();

    selected
}

pub fn extract_values_prompt(
    input: &str,
) -> Result<(String, String), Box<dyn Error + Send + Sync>> {
    let image_prompt_re = Regex::new(r"(?m)^Image Prompt:\s*(.+)")?;
    let model_re = Regex::new(r"(?m)^Model:\s*(.+)")?;

    let image_prompt = image_prompt_re
        .captures(input)
        .and_then(|cap| cap.get(1).map(|m| m.as_str()))
        .unwrap_or_default()
        .to_string();
    let model = model_re
        .captures(input)
        .and_then(|cap| cap.get(1).map(|m| m.as_str()))
        .unwrap_or_default()
        .to_string();

    Ok((image_prompt, model))
}

pub fn format_instructions(prompt: &Prompt) -> String {
    format!(
        r#"
Custom Instructions: {}
Lore: {}
Knowledge: {}
Style: {}
Adjectives: {}
"#,
        prompt.custom_instructions, prompt.lore, prompt.knowledge, prompt.style, prompt.adjectives
    )
}

pub async fn fetch_metadata(uri: &str) -> Option<Value> {
    if let Some(ipfs_hash) = uri.strip_prefix("ipfs://") {
        let client = Client::new();
        let url = format!("{}ipfs/{}", INFURA_GATEWAY, ipfs_hash);
        if let Ok(response) = client.get(&url).send().await {
            if let Ok(json) = response.json::<Value>().await {
                return Some(json);
            }
        }
    }
    None
}

pub fn extract_values_spectate(input: &str) -> Result<String, Box<dyn Error + Send + Sync>> {
    let comment_re = Regex::new(r"(?m)^Comment:\s*(.+)")?;
    let model_re = Regex::new(r"(?m)^Model:\s*(\d+)")?;
    let scene_re = Regex::new(r"(?m)^Scene:\s*(\d+)")?;
    let chat_context_re = Regex::new(r"(?m)^ChatContext:\s*(\d+)")?;
    let appearance_re = Regex::new(r"(?m)^Appearance:\s*(\d+)")?;
    let collections_re = Regex::new(r"(?m)^Collections:\s*(\d+)")?;
    let personality_re = Regex::new(r"(?m)^Personality:\s*(\d+)")?;
    let training_re = Regex::new(r"(?m)^Training:\s*(\d+)")?;
    let tokenizer_re = Regex::new(r"(?m)^Tokenizer:\s*(\d+)")?;
    let lora_re = Regex::new(r"(?m)^Lora:\s*(\d+)")?;
    let sprite_re = Regex::new(r"(?m)^Sprite:\s*(\d+)")?;
    let global_re = Regex::new(r"(?m)^Global:\s*(\d+)")?;

    let comment = comment_re
        .captures(input)
        .and_then(|cap| cap.get(1).map(|m| m.as_str()))
        .unwrap_or_default()
        .to_string();

    let model: U256 = model_re
        .captures(input)
        .and_then(|cap| cap.get(1))
        .and_then(|m| U256::from_dec_str(m.as_str()).ok())
        .unwrap_or(U256::zero());

    let scene: U256 = scene_re
        .captures(input)
        .and_then(|cap| cap.get(1))
        .and_then(|m| U256::from_dec_str(m.as_str()).ok())
        .unwrap_or(U256::zero());

    let chat_context: U256 = chat_context_re
        .captures(input)
        .and_then(|cap| cap.get(1))
        .and_then(|m| U256::from_dec_str(m.as_str()).ok())
        .unwrap_or(U256::zero());

    let appearance: U256 = appearance_re
        .captures(input)
        .and_then(|cap| cap.get(1))
        .and_then(|m| U256::from_dec_str(m.as_str()).ok())
        .unwrap_or(U256::zero());

    let collections: U256 = collections_re
        .captures(input)
        .and_then(|cap| cap.get(1))
        .and_then(|m| U256::from_dec_str(m.as_str()).ok())
        .unwrap_or(U256::zero());

    let personality: U256 = personality_re
        .captures(input)
        .and_then(|cap| cap.get(1))
        .and_then(|m| U256::from_dec_str(m.as_str()).ok())
        .unwrap_or(U256::zero());

    let training: U256 = training_re
        .captures(input)
        .and_then(|cap| cap.get(1))
        .and_then(|m| U256::from_dec_str(m.as_str()).ok())
        .unwrap_or(U256::zero());

    let tokenizer: U256 = tokenizer_re
        .captures(input)
        .and_then(|cap| cap.get(1))
        .and_then(|m| U256::from_dec_str(m.as_str()).ok())
        .unwrap_or(U256::zero());

    let lora: U256 = lora_re
        .captures(input)
        .and_then(|cap| cap.get(1))
        .and_then(|m| U256::from_dec_str(m.as_str()).ok())
        .unwrap_or(U256::zero());

    let sprite: U256 = sprite_re
        .captures(input)
        .and_then(|cap| cap.get(1))
        .and_then(|m| U256::from_dec_str(m.as_str()).ok())
        .unwrap_or(U256::zero());

    let global: U256 = global_re
        .captures(input)
        .and_then(|cap| cap.get(1))
        .and_then(|m| U256::from_dec_str(m.as_str()).ok())
        .unwrap_or(U256::zero());

    Ok(to_string(&json!({
        "comment": comment,
        "model": model,
        "scene": scene,
        "chatContext": chat_context,
        "appearance":appearance,
        "collections": collections,
        "personality": personality,
        "training": training,
        "tokenizer": tokenizer,
        "lora": lora,
        "spriteSheet":sprite ,
        "global":global ,
    }))
    .unwrap())
}
