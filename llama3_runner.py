import sys
import json
import subprocess
import os

COMMON_PATHS = [
    os.path.join(os.getcwd(), "bin"),
    "/usr/local/bin",
    "/usr/bin",
    "/bin",
    "/usr/sbin",
    "/sbin",
    os.path.expanduser("~/bin"),
    "/opt/render/project/src/bin",
]

def find_ollama():
    for path in COMMON_PATHS:
        full_path = os.path.join(path, "ollama")
        print(f"Checking path: {full_path}")
        if os.path.isfile(full_path) and os.access(full_path, os.X_OK):
            print(f"Found ollama binary at: {full_path}")
            return full_path
    
    print("ollama binary not found in the system")
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
    print(f"Updated PATH: {os.environ.get('PATH')}")
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
