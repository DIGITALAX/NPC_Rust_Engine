import sys
import json
import subprocess
import os

def main():
    if len(sys.argv) != 2:
        print("Uso: python3 llama3_runner.py <prompt>")
        sys.exit(1)

    prompt = sys.argv[1]
    
    ollama_path = os.environ.get('OLLAMA_PATH') or 'ollama'

    print(f"Usando Ollama desde: {ollama_path}")
    
    try:
        result = subprocess.run(
            [ollama_path, 'run', "llama3", prompt],
            capture_output=True,
            text=True,
            check=True
        )
        response = result.stdout.strip()
        print(json.dumps({"response": response}))
    except subprocess.CalledProcessError as e:
        print(f"Error ejecutando el modelo: {e.stderr}")
        sys.exit(1)
    except FileNotFoundError as e:
        print(f"Error: No se encontró Ollama. Asegúrate de que esté instalado y en el PATH.")
        sys.exit(1)
    except Exception as e:
        print(f"Ocurrió un error inesperado: {e}")
        sys.exit(1)

if __name__ == "__main__":
    main()