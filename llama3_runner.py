import sys
import json
import subprocess
import os
import time

def is_service_up(port):
    try:
        result = subprocess.run(['curl', '-s', '-o', '/dev/null', '-w', '%{http_code}', f'http://localhost:{port}'], capture_output=True, text=True)
        return result.stdout.strip() == "200"
    except Exception as e:
        print(f"Error checking service status: {e}")
        return False

def start_service(ollama_path, port):
    try:
        env = os.environ.copy()
        env['OLLAMA_HOST'] = f'http://127.0.0.1:{port}'
        ollama_process = subprocess.Popen([ollama_path, 'serve'], env=env, stderr=subprocess.PIPE)
        time.sleep(5)  # Esperar unos segundos para asegurarse de que el servicio esté activo
        stderr = ollama_process.stderr.read().decode('utf-8')
        if 'address already in use' in stderr:
            print(f"Error: Port {port} already in use.")
            ollama_process.terminate()
            return None
        return ollama_process
    except Exception as e:
        print(f"Error starting ollama service: {e}")
        return None

def main():
    if len(sys.argv) != 2:
        print("Usage: python3 llama3_runner.py <prompt>")
        sys.exit(1)

    prompt = sys.argv[1]
    ollama_path = os.path.join(os.path.expanduser("~"), "project", "src", "ollama")

    print(f"Current working directory: {os.getcwd()}")
    print(f"Using ollama from: {ollama_path}")

    os.environ["PATH"] = os.path.dirname(ollama_path) + os.pathsep + os.environ.get("PATH", "")
    print(f"Updated PATH: {os.environ.get('PATH')}")

    if not os.path.isfile(ollama_path):
        print(f"Error: ollama binary not found at {ollama_path}")
        sys.exit(1)
    if not os.access(ollama_path, os.X_OK):
        print(f"Error: ollama binary is not executable at {ollama_path}")
        sys.exit(1)

    port = 11435  # Puerto alternativo para redirigir
    if not is_service_up(port):
        ollama_process = start_service(ollama_path, port)
    else:
        ollama_process = None

    if not is_service_up(port):
        print(f"Error: could not connect to ollama app on port {port}, is it running?")
        if ollama_process:
            ollama_process.terminate()
        sys.exit(1)

    try:
        result = subprocess.run(
            [ollama_path, 'run', "llama3", '--host', f'http://127.0.0.1:{port}', prompt],
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
    finally:
        if ollama_process:
            ollama_process.terminate()

if __name__ == "__main__":
    main()
