import sys
import json
import subprocess
import os

def main():
    if len(sys.argv) != 2:
        print("Usage: python3 llama3_runner.py <prompt>")
        sys.exit(1)

    prompt = sys.argv[1]
    ollama_path = "/tmp/ollama/ollama"

    print(f"Current working directory: {os.getcwd()}")
    print(f"Using ollama from: {ollama_path}")
    print(f"PATH: {os.environ.get('PATH')}")

    if not os.path.isfile(ollama_path):
        print(f"Error: ollama binary not found at {ollama_path}")
        sys.exit(1)
    if not os.access(ollama_path, os.X_OK):
        print(f"Error: ollama binary is not executable at {ollama_path}")
        sys.exit(1)


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
        print(f"Error: {e}")
        sys.exit(1)
    except Exception as e:
        print(f"An unexpected error occurred: {e}")
        sys.exit(1)

if __name__ == "__main__":
    main()
