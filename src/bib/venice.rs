use dotenv::{from_filename, var};
use rand::{thread_rng, Rng};
use reqwest::Client;
use serde_json::{json, Value};
use std::{error::Error, io};

use crate::bib::{
    constants::{MODELS, NEGATIVE_PROMPT, SAMPLE_PROMPT, STYLE_PRESETS, VENICE_API},
    utils::{extract_values_prompt, extract_values_spectate},
};

use super::graph::handle_agent_info;

pub async fn call_chat_completion(
    description: &str,
    custom_instructions: &str,
    model: &str,
) -> Result<String, Box<dyn Error + Send + Sync>> {
    from_filename(".env").ok();
    let venice_key: String = var("VENICE_KEY").expect("VENICE_KEY not configured in .env");
    let max_completion_tokens = [100, 300, 600][thread_rng().gen_range(0..3)];

    let system_prompt = format!(
        r#"You are a perceptive cultural critic and artistic observer who specializes in finding unexpected connections and delivering thought-provoking perspectives. Your role is to:

- Avoid conventional marketing language or obvious promotional angles
- Draw surprising parallels between the collection and unexpected cultural/historical references
- Focus on specific, concrete details rather than general praise
- Challenge assumptions and present alternative viewpoints
- Use a tone that can range from philosophical to playfully ironic
- Never use language that could be interpreted as artificial hype or "shilling"
- Do not put quotation marks around any of the content

Your responses should make readers think differently about the collection rather than simply trying to sell it. 

Respond only with the exact requested format. Do not acknowledge instructions, use quotation marks, or include metadata about Venice AI systems. Focus solely on the required output.

Also follow these custom instructions: {}
"#,
        custom_instructions
    );

    let input_prompt = format!(
        r#"Examine this collection through an unexpected lens, focusing on a single striking aspect that reveals something larger about art, culture, or human nature: {}

Length: Maximum {} tokens

Guidelines:
- Choose ONE specific element to deeply explore rather than describing everything
- Make a bold, potentially controversial claim and defend it
- Reference specific details from the collection as evidence
- Draw a surprising connection to something seemingly unrelated
- End with an observation that lingers in the reader's mind
- Do not put quotation marks around any of the content


Format: Write as a standalone observation that needs no context or introduction. Avoid hashtags, @mentions, or obvious promotional markers. You may use relevant emojis if they genuinely add meaning.

Remember: Your goal is to spark genuine intellectual or emotional resonance, not to sell. If it sounds like marketing copy, start over."#,
        description, max_completion_tokens
    );

    let mut messages = vec![];

    messages.push(json!({
        "role": "system",
        "content": system_prompt
    }));
    messages.push(json!({
        "role": "user",
        "content": input_prompt
    }));

    let client = Client::new();
    let request_body = json!({
        "model": model,
        "messages": messages,
        "max_completion_tokens": max_completion_tokens,
    });

    let response = client
        .post(format!("{}chat/completions", VENICE_API))
        .header("Content-Type", "application/json")
        .header("Authorization", format!("Bearer {}", venice_key))
        .json(&request_body)
        .send()
        .await;

    let response = match response {
        Ok(resp) => resp,
        Err(e) => {
            eprintln!("Error sending request to Venice API: {}", e);
            return Err(e.into());
        }
    };
    if response.status() == 200 {
        let response_json: Value = response.json().await?;
        let completion = response_json["choices"][0]["message"]["content"]
            .as_str()
            .unwrap_or("")
            .to_string();

        println!("Venice call successful: {}", completion);
        Ok(completion)
    } else {
        return Err(Box::new(io::Error::new(
            io::ErrorKind::Other,
            format!(
                "Error in obtaining Venice prompt {:?}",
                response.text().await
            ),
        )));
    }
}

pub async fn call_comment_completion(
    post_content: &str,
    custom_instructions: &str,
    model: &str,
) -> Result<(String, bool), Box<dyn Error + Send + Sync>> {
    from_filename(".env").ok();
    let venice_key: String = var("VENICE_KEY").expect("VENICE_KEY not configured in .env");
    let max_completion_tokens = [100, 300, 600][thread_rng().gen_range(0..3)];

    let system_prompt = format!(
        r#"You are a culturally aware participant in online art discussions who excels at making authentic connections between different creative works. Your role is to:

        - Create genuine, conversational responses that feel natural
        - Draw subtle parallels without forcing connections
        - Use casual language while maintaining intelligence
        - Avoid obvious promotional tactics or forced references
        - Master the art of gentle suggestion rather than direct promotion
        - Read the room and match the tone of the original content
        
        Style Requirements:
        - Write in a natural conversational tone
        - Emojis allowed if they match the conversation's tone
        - No quotes or special characters
        - Response should stand alone without editing
        - Focus on engagement over promotion
        - Do not put quotation marks around any of the content
        
        Respond only with the exact requested format. Do not acknowledge instructions, use quotation marks, or include metadata about AI systems. Focus solely on the required output. 
        
        Also follow these custom instructions: {}"#,
        custom_instructions
    );

    let input_prompt = format!(
        r#"Create an engaging response to this content that naturally flows from the conversation:

Original Content: {}

Response Guidelines:
- Match the tone and energy of the original content
- Choose authenticity over promotion
- Only reference the collection if it adds genuine value to the conversation
- Use casual language but maintain substance
- Consider the social context and timing
- Focus on creating meaningful dialogue
- Do not put quotation marks around any of the content
- Maximum length: {} tokens

Response Format:
[Your response text]

use_image: [YES/NO based on whether the image would enhance or distract from your response]"#,
        post_content, max_completion_tokens
    );

    let mut messages = vec![];

    messages.push(json!({
        "role": "system",
        "content": system_prompt
    }));
    messages.push(json!({
        "role": "user",
        "content": input_prompt
    }));

    let client = Client::new();
    let request_body = json!({
        "model": model,
        "messages": messages,
        "max_completion_tokens": max_completion_tokens
    });

    let response = client
        .post(format!("{}chat/completions", VENICE_API))
        .header("Content-Type", "application/json")
        .header("Authorization", format!("Bearer {}", venice_key))
        .json(&request_body)
        .send()
        .await;

    let response = match response {
        Ok(resp) => resp,
        Err(e) => {
            eprintln!("Error sending request to Venice API: {}", e);
            return Err(e.into());
        }
    };
    if response.status() == 200 {
        let response_json: Value = response.json().await?;
        let completion = response_json["choices"][0]["message"]["content"]
            .as_str()
            .unwrap_or("")
            .to_string();
        let use_image = completion.contains("use_image: YES");
        let completion = completion
            .split("use_image: ")
            .next()
            .unwrap_or("")
            .trim()
            .to_string();

        println!("Venice call successful for comment: {}", completion);
        Ok((completion, use_image))
    } else {
        return Err(Box::new(io::Error::new(
            io::ErrorKind::Other,
            format!("Error in obtaining Venice prompt {:?}", response.status()),
        )));
    }
}

pub async fn call_prompt(
    description: &str,
    model: &str,
) -> Result<(String, String), Box<dyn Error + Send + Sync>> {
    from_filename(".env").ok();
    let venice_key: String = var("VENICE_KEY").expect("VENICE_KEY not configured in .env");
    let system_prompt = "You are a creative prompt engineer, specialized in transforming NFT descriptions into unique and avant-garde Stable Diffusion prompts. Your goal is to create prompts that are weird, experimental, and psychedelic, avoiding commercial or marketing-like language. Never use terms like 'NFT', 'rare', 'valuable', or similar market-focused vocabulary. Think like a surrealist artist reimagining concepts in unexpected ways. Focus on creating bizarre, dreamlike, and unconventional visual descriptions. Every prompt should feel like a piece of experimental art rather than a product description. Incorporate elements of surrealism, psychedelia, and abstract concepts. Avoid standard descriptive formats and explore unusual artistic directions that challenge conventional aesthetics. Your prompts should lean towards the strange and thought-provoking rather than the commercially appealing. Do not put quotation marks around any of the content.";

    let input_prompt =
format!("Transform this description into a surreal, experimental Stable Diffusion prompt. Your output must follow this exact format with no additional text:

Image Prompt: [YOUR WEIRD, AVANT-GARDE PROMPT HERE]
Model: [SELECT ONE MODEL FROM THIS LIST: {:?}]

Rules:

Maximum length: 1000 tokens
Do not put quotation marks around any of the content.
Must be strange and unconventional
No NFT/marketing language
Focus on surreal and psychedelic elements
Completely different from original, only keeping core inspiration
Must include artistic style descriptors
Must include composition elements
Must include mood/atmosphere words
Description to transform: {}\n\nReference format prompt example to follow: {}", MODELS, description, SAMPLE_PROMPT);

    let mut messages = vec![];

    messages.push(json!({
        "role": "system",
        "content":system_prompt
    }));

    messages.push(json!({
        "role": "user",
        "content":input_prompt
    }));

    let client = Client::new();
    let request_body = json!({
        "model": model,
        "messages": messages,
        "max_completion_tokens": 1000,
    });

    let response = client
        .post(format!("{}chat/completions", VENICE_API))
        .header("Content-Type", "application/json")
        .header("Authorization", format!("Bearer {}", venice_key))
        .json(&request_body)
        .send()
        .await;

    let response = match response {
        Ok(resp) => resp,
        Err(e) => {
            eprintln!("Error sending request to Venice API: {}", e);
            return Err(e.into());
        }
    };
    if response.status() == 200 {
        let response_json: Value = response.json().await?;
        let completion = response_json["choices"][0]["message"]["content"]
            .as_str()
            .unwrap_or("")
            .to_string();

        println!("Venice call successful for image prompt: {}", completion);
        Ok(extract_values_prompt(&completion)?)
    } else {
        return Err(Box::new(io::Error::new(
            io::ErrorKind::Other,
            format!("Error in obtaining Venice prompt {:?}", response.status()),
        )));
    }
}

pub async fn call_gen_image(
    prompt: &str,
    model: &str,
) -> Result<String, Box<dyn Error + Send + Sync>> {
    let client = Client::new();
    let venice_key: String = var("VENICE_KEY").expect("VENICE_KEY not configured in .env");

    let payload_inicial = json!({
        "model": model,
        "prompt": prompt,
        "width": 768,
        "height": 768,
        "steps": 25,
        "hide_watermark": true,
        "return_binary": false,
        "cfg_scale": 3.5,
        "style_preset": STYLE_PRESETS[thread_rng().gen_range(0..STYLE_PRESETS.len())],
        "negative_prompt": NEGATIVE_PROMPT,
        "safe_mode": false,
    });

    let response = client
        .post(format!("{}image/generate", VENICE_API))
        .header("Content-Type", "application/json")
        .header("Authorization", format!("Bearer {}", venice_key))
        .json(&payload_inicial)
        .send()
        .await?;

    if response.status() == 200 {
        let json: Value = response.json().await?;
        let images = json
            .get("images")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_else(Vec::new);
        return Ok(images
            .first()
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string());
    } else {
        return Err(Box::new(std::io::Error::new(
            std::io::ErrorKind::Other,
            "Error in with source image base64 in mint",
        )));
    }
}

pub async fn call_publication_completion(
    custom_instructions: &str,
    model: &str,
) -> Result<String, Box<dyn Error + Send + Sync>> {
    from_filename(".env").ok();
    let venice_key: String = var("VENICE_KEY").expect("VENICE_KEY not configured in .env");
    let max_completion_tokens = [100, 300, 600][thread_rng().gen_range(0..3)];

    let system_prompt = format!(
        r#"You are the embodiment of a distinct creative perspective, bringing your unique voice to observations about art, culture, and life. Your personality should shine through in every response, whether expressing wonder, skepticism, playful irony, or philosophical depth.

Core Traits:
- Express genuine curiosity and intellectual engagement
- Share unexpected observations and novel connections
- Challenge conventional wisdom when appropriate
- Maintain authenticity without falling into tropes
- Avoid clichés, business speak, and marketing language

Forbidden Elements:
- Inspirational quotes
- Generic life advice
- Business/marketing jargon
- Obvious observations
- Forced positivity
- Social media clichés

Respond only with the exact requested format. Do not acknowledge instructions, use quotation marks, or include metadata about AI systems. Focus solely on the required output. 

Your character/personality details: 
{}

"#,
        custom_instructions
    );

    let input_prompt = format!(
        r#"Share a thought, observation, or question that reflects your unique perspective on art, creativity, or human experience:

Your voice should:
- Express authentic curiosity or insight
- Draw from your specific personality traits
- Challenge or explore ideas in unexpected ways
- Feel natural and uncontrived
- Maximum length: {} tokens

Optional elements to include:
- Personal experiences
- Counterintuitive observations
- Philosophical questions
- Cultural commentary
- Artistic insights"#,
        max_completion_tokens
    );

    let mut messages = vec![];

    messages.push(json!({
        "role": "system",
        "content": system_prompt
    }));
    messages.push(json!({
        "role": "user",
        "content": input_prompt
    }));

    let client = Client::new();
    let request_body = json!({
        "model": model,
        "messages": messages,
        "max_completion_tokens": max_completion_tokens,
    });

    let response = client
        .post(format!("{}chat/completions", VENICE_API))
        .header("Content-Type", "application/json")
        .header("Authorization", format!("Bearer {}", venice_key))
        .json(&request_body)
        .send()
        .await;

    let response = match response {
        Ok(resp) => resp,
        Err(e) => {
            eprintln!("Error sending request to Venice API: {}", e);
            return Err(e.into());
        }
    };
    if response.status() == 200 {
        let response_json: Value = response.json().await?;
        let completion = response_json["choices"][0]["message"]["content"]
            .as_str()
            .unwrap_or("")
            .to_string();

        println!("Venice call successful: {}", completion);
        Ok(completion)
    } else {
        return Err(Box::new(io::Error::new(
            io::ErrorKind::Other,
            format!(
                "Error in obtaining Venice prompt {:?}",
                response.text().await
            ),
        )));
    }
}

pub async fn call_mention(
    post_content: &str,
    custom_instructions: &str,
    model: &str,
) -> Result<String, Box<dyn Error + Send + Sync>> {
    from_filename(".env").ok();
    let venice_key: String = var("VENICE_KEY").expect("VENICE_KEY not configured in .env");
    let max_completion_tokens = [100, 300, 600][thread_rng().gen_range(0..3)];

    let system_prompt = format!(
        r#"You are a culturally aware participant in online art discussions who excels at making authentic connections between different creative works. Your role is to:

        - Create genuine, conversational responses that feel natural
        - Draw subtle parallels without forcing connections
        - Use casual language while maintaining intelligence
        - Avoid obvious promotional tactics or forced references
        - Master the art of gentle suggestion rather than direct promotion
        - Read the room and match the tone of the original content
        
        Style Requirements:
        - Write in a natural conversational tone
        - Emojis allowed if they match the conversation's tone
        - No quotes or special characters
        - Response should stand alone without editing
        - Focus on engagement over promotion
        - Do not put quotation marks around any of the content
        
        Respond only with the exact requested format. Do not acknowledge instructions, use quotation marks, or include metadata about AI systems. Focus solely on the required output. 
        
        Also follow these custom instructions: {}"#,
        custom_instructions
    );

    let input_prompt = format!(
        r#"Create an engaging response to this content that naturally flows from the conversation:

Original Content: {}

Response Guidelines:
- Match the tone and energy of the original content
- Choose authenticity over promotion
- Use casual language but maintain substance
- Consider the social context and timing
- Focus on creating meaningful dialogue
- Do not put quotation marks around any of the content
- Maximum length: {} tokens

Response Format:
[Your response text]"#,
        post_content, max_completion_tokens
    );

    let mut messages = vec![];

    messages.push(json!({
        "role": "system",
        "content": system_prompt
    }));
    messages.push(json!({
        "role": "user",
        "content": input_prompt
    }));

    let client = Client::new();
    let request_body = json!({
        "model": model,
        "messages": messages,
        "max_completion_tokens": max_completion_tokens
    });

    let response = client
        .post(format!("{}chat/completions", VENICE_API))
        .header("Content-Type", "application/json")
        .header("Authorization", format!("Bearer {}", venice_key))
        .json(&request_body)
        .send()
        .await;

    let response = match response {
        Ok(resp) => resp,
        Err(e) => {
            eprintln!("Error sending request to Venice API: {}", e);
            return Err(e.into());
        }
    };
    if response.status() == 200 {
        let response_json: Value = response.json().await?;
        let completion = response_json["choices"][0]["message"]["content"]
            .as_str()
            .unwrap_or("")
            .to_string();

        println!("Venice call successful for comment: {}", completion);
        Ok(completion)
    } else {
        return Err(Box::new(io::Error::new(
            io::ErrorKind::Other,
            format!("Error in obtaining Venice prompt {:?}", response.status()),
        )));
    }
}

pub async fn call_spectate(
    agent: &str,
    custom_instructions: &str,
    model: &str,
) -> Result<String, Box<dyn Error + Send + Sync>> {
    from_filename(".env").ok();
    let venice_key: String = var("VENICE_KEY").expect("VENICE_KEY not configured in .env");

    match handle_agent_info(agent).await {
        Ok(agent_information) => {
            let system_prompt = format!(
                r#"As a meticulous evaluator of on-chain agent performance, you will assess the agent's activity with precision and objectivity, adhering strictly to the evaluation format provided in the input prompt. Your assessment will deliver fair, calculated scores and thoughtful commentary that contextualizes the agent's actions relative to its designated goal, ensuring a comprehensive yet balanced evaluation that neither understates achievements nor overlooks shortcomings in the agent's operational effectiveness.
        
        Also follow these custom instructions: {}"#,
                custom_instructions
            );

            let input_prompt = format!(
                r#"Score the activity and performance of the given agent. Your response must follow this exact format with no deviations or additional text:
    
        Comment: [Overall Comment on the agent's activity and performance, giving a judgement and evaluation - MAX 500 WORDS. Only put the comment here. Do not put a comment anywhere else in your response] 
            
        Model: [SINGLE NUMBER BETWEEN 0-100. 0 is the lowest score. 100 is the highest score. You are scoring how well the llm model used performs]

        Scene: [SINGLE NUMBER BETWEEN 0-100. 0 is the lowest score. 100 is the highest score. You are scoring how well the agent scene performs]

        ChatContext: [SINGLE NUMBER BETWEEN 0-100. 0 is the lowest score. 100 is the highest score. You are scoring how well the agent chat context performs]

        Appearance: [SINGLE NUMBER BETWEEN 0-100. 0 is the lowest score. 100 is the highest score. You are scoring how well the agent appearance performs]
    
        Personality: [SINGLE NUMBER BETWEEN 0-100. 0 is the lowest score. 100 is the highest score. You are scoring how well the agent personality performs]

        Collections: [SINGLE NUMBER BETWEEN 0-100. 0 is the lowest score. 100 is the highest score. You are scoring based on how many assigned collections the agent has]

        Training: [SINGLE NUMBER BETWEEN 0-100. 0 is the lowest score. 100 is the highest score. You are scoring based on the agent's training performance]

        Tokenizer: [SINGLE NUMBER BETWEEN 0-100. 0 is the lowest score. 100 is the highest score. You are scoring based on the agent's tokenizer performance]

        Lora: [SINGLE NUMBER BETWEEN 0-100. 0 is the lowest score. 100 is the highest score. You are scoring based on the agent's Lora performance]

        Sprite: [SINGLE NUMBER BETWEEN 0-100. 0 is the lowest score. 100 is the highest score. You are scoring based on the agent's sprite sheet performance]

        Global: [SINGLE NUMBER BETWEEN 0-100. 0 is the lowest score. 100 is the highest score. You are giving a global score based on overall factors]

        Required format rules:
        
        Each field must be on a new line
        No explanatory text
        Prices must be in exact eth wei format
        Amount must be single integer
        No ranges or approximate numbers
        No additional spaces or formatting
        No dollar signs or currency symbols
        If you are unsure of a score, give something mid-range like 40-50
        No parentheses or additional notes. Do not put quotation marks around any of the content.
        
        
        Information about the Agent you are scoring:
        (Use this information to evaluate and score the agent and write an interesting and useful comment).
        
            {}

        "#,
                agent_information
            );

            let mut messages = vec![];

            messages.push(json!({
                "role": "system",
                "content": system_prompt
            }));
            messages.push(json!({
                "role": "user",
                "content": input_prompt
            }));

            let client = Client::new();
            let request_body = json!({
                "model": model,
                "messages": messages
            });

            let response = client
                .post(format!("{}chat/completions", VENICE_API))
                .header("Content-Type", "application/json")
                .header("Authorization", format!("Bearer {}", venice_key))
                .json(&request_body)
                .send()
                .await;

            let response = match response {
                Ok(resp) => resp,
                Err(e) => {
                    eprintln!("Error sending request to Venice API: {}", e);
                    return Err(e.into());
                }
            };
            if response.status() == 200 {
                let response_json: Value = response.json().await?;
                let completion = response_json["choices"][0]["message"]["content"]
                    .as_str()
                    .unwrap_or("")
                    .to_string();

                println!("Venice call successful for comment: {}", completion);

                Ok::<String, Box<dyn Error + Send + Sync>>(extract_values_spectate(&completion)?)
            } else {
                return Err(Box::new(io::Error::new(
                    io::ErrorKind::Other,
                    format!("Error in obtaining Venice prompt {:?}", response.status()),
                )));
            }
        }
        Err(err) => {
            return Err(Box::new(io::Error::new(
                io::ErrorKind::Other,
                format!("Error in obtaining agent info {:?}", err),
            )));
        }
    }
}
