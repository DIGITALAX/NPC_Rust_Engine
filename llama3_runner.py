import sys
import json
import subprocess
import os
import time

def is_service_up(port):
    result = subprocess.run(['curl', '-s', '-o', '/dev/null', '-w', '%{http_code}', f'http://localhost:{port}'], capture_output=True, text=True)
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

    port = find_available_port()
    if port is None:
        print("Error: no available ports found")
        sys.exit(1)

    if not is_service_up(port):
        ollama_process = subprocess.Popen([ollama_path, 'serve', '--port', str(port)])
        time.sleep(5)  
    else:
        ollama_process = None

    if not is_service_up(port):
        print(f"Error: could not connect to ollama app on port {port}, is it running?")
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
