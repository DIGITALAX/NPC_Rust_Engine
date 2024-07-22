import sys
import json
import subprocess
import os

def find_ollama():
    possible_paths = [
        '/opt/render/ollama_bin/ollama',
        os.path.join(os.path.expanduser('~'), 'ollama_bin', 'ollama'),
        os.path.join(os.path.dirname(os.path.abspath(__file__)), 'ollama_bin', 'ollama'),
        os.path.join(os.getcwd(), 'ollama_bin', 'ollama')
    ]
    for path in possible_paths:
        if os.path.isfile(path):
            print(f"Found ollama binary at: {path}")
            return path
    return None

def main():
    if len(sys.argv) != 2:
        print("Usage: python3 llama3_runner.py <prompt>")
        sys.exit(1)

    prompt = sys.argv[1]
    ollama_path = find_ollama()

    print(f"Current working directory: {os.getcwd()}")
    print(f"PATH: {os.environ.get('PATH')}")
    print(f"Using ollama from: {ollama_path}")

    if not ollama_path:
        print("Error: ollama binary not found")
        sys.exit(1)

    ollama_dir = os.path.dirname(ollama_path)
    os.environ["PATH"] = ollama_dir + os.pathsep + os.environ.get('PATH', '')


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
