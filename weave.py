import wandb
import weave
import openai
from transformers import AutoModelForCausalLM, AutoTokenizer

client = openai.OpenAI(
    api_key='fake',
    base_url="http://localhost:1234",
)

@weave.op()
def extract_dinos(sentence: str) -> dict:
    response = client.chat.completions.create(
        model="gpt-4o",
        messages=[
            {
                "role": "system",
                "content": """In JSON format extract a list of `dinosaurs`, with their `name`, 
their `common_name`, and whether its `diet` is a herbivore or carnivore"""
            },
            {
                "role": "user",
                "content": sentence
            }
            ],
            response_format={ "type": "json_object" }
        )
    return response.choices[0].message.content
