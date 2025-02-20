use crate::bib::{types::{LensType, Prompt}, constants::INFURA_GATEWAY};
use ethers::types::U256;
use rand::{seq::SliceRandom, thread_rng, Rng};
use regex::Regex;
use reqwest::Client;
use serde_json::Value;
use std::error::Error;
use strum::IntoEnumIterator;

pub fn between(min: f32, max: f32) -> f32 {
    let mut rng = rand::thread_rng();
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