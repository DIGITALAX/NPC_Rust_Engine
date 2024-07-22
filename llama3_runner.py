import sys
import json
import subprocess
import os
import time

def is_service_up(port):
    result = subprocess.run(['curl', '-s', '-o', '/dev/null', '-w', '%{{http_code}}'.format(port=port), 'http://localhost:{}'.format(port)], capture_output=True, text=True)
    return result.stdout.strip() == "200"

def find_available_port(start_port=11411, max_attempts=100):
    for i in range(max_attempts):
        port = start_port + i
        if not is_service_up(port):
            return port
    return None

def main():
    if len(sys.argv) != 2:
        print("Usage: python3 llama3_runner.py <prompt>")
        sys.exit(1)

    prompt = sys.argv[1]
    ollama_path = os.path.join(os.path.expanduser("~"), "project", "src", "ollama")

    print("Current working directory: {}".format(os.getcwd()))
    print("Using ollama from: {}".format(ollama_path))

    os.environ["PATH"] = os.path.dirname(ollama_path) + os.pathsep + os.environ.get("PATH", "")
    print("Updated PATH: {}".format(os.environ.get('PATH')))

    if not os.path.isfile(ollama_path):
        print("Error: ollama binary not found at {}".format(ollama_path))
        sys.exit(1)
    if not os.access(ollama_path, os.X_OK):
        print("Error: ollama binary is not executable at {}".format(ollama_path))
        sys.exit(1)

    port = find_available_port()
    if port is None:
        print("Error: no available ports found")
        sys.exit(1)

    if not is_service_up(port):
        ollama_process = subprocess.Popen([ollama_path, 'serve', '--port', str(port)])
        time.sleep(5)  # Esperar unos segundos para asegurarse de que el servicio esté activo
    else:
        ollama_process = None

    if not is_service_up(port):
        print("Error: could not connect to ollama app on port {}, is it running?".format(port))
        if ollama_process:
            ollama_process.terminate()
        sys.exit(1)

    try:
        result = subprocess.run(
            [ollama_path, 'run', '--port', str(port), "llama3", prompt],
            capture_output=True,
            text=True,
            check=True
        )
        response = result.stdout.strip()
        print(json.dumps({"response": response}))
    except subprocess.CalledProcessError as e:
        print("Error ejecutando el modelo: {}".format(e.stderr))
        sys.exit(1)
    except FileNotFoundError as e:
        print("Error: {}".format(e))
        sys.exit(1)
    except Exception as e:
        print("An unexpected error occurred: {}".format(e))
        sys.exit(1)
    finally:
        if ollama_process:
            ollama_process.terminate()

if __name__ == "__main__":
    main()
