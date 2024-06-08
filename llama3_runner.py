import sys
import json
import subprocess

def main():
    if len(sys.argv) != 2:
        print("Usage: python3 llama_runner.py <prompt>")
        sys.exit(1)
    
    prompt = sys.argv[1]

    try:
        result = subprocess.run(
            ['ollama', 'run', "llama3", '--prompt', prompt],
            capture_output=True,
            text=True,
            check=True
        )
        response = result.stdout.strip()
        print(json.dumps({"response": response}))
    except subprocess.CalledProcessError as e:
        print(f"Error ejecutando el modelo: {e.stderr}")
        sys.exit(1)

if __name__ == "__main__":
    main()
