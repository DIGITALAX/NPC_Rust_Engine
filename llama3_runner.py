import sys
import json
import subprocess
import os

OLLAMA_DIR = "/opt/render/ollama_bin"
OLLAMA_FILE = os.path.join(OLLAMA_DIR, "ollama")

def find_ollama():
    print(f"Checking path: {OLLAMA_FILE}")
    if os.path.isfile(OLLAMA_FILE):
        print(f"Found ollama binary at: {OLLAMA_FILE}")
        return OLLAMA_FILE
    return None

def main():
    if len(sys.argv) != 2:
        print("Usage: python3 llama3_runner.py <prompt>")
        sys.exit(1)

    prompt = sys.argv[1]
    ollama_path = find_ollama()

    if not ollama_path:
        print("Error: ollama binary not found")
        sys.exit(1)

    ollama_dir = os.path.dirname(ollama_path)
    os.environ["PATH"] = ollama_dir + os.pathsep + os.environ.get('PATH', '')

    print(f"Current working directory: {os.getcwd()}")
    print(f"PATH: {os.environ.get('PATH')}")
    print(f"Using ollama from: {ollama_path}")

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
