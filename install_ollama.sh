#!/bin/bash

OLLAMA_DIR="/opt/render/ollama_bin"
OLLAMA_FILE="$OLLAMA_DIR/ollama"
mkdir -p $OLLAMA_DIR

echo "Downloading ollama..."
curl -L https://ollama.com/download/ollama-linux-amd64 -o $OLLAMA_FILE
chmod +x $OLLAMA_FILE
 
if [ ! -f $OLLAMA_FILE ]; then
    echo "ollama could not be downloaded"
    exit 1
fi

echo "Verifying ollama installation..."
if ! $OLLAMA_FILE --version &> /dev/null; then
    echo "ollama could not be executed"
    exit 1
fi

echo "ollama installed successfully in $OLLAMA_FILE"

export PATH=$OLLAMA_DIR:$PATH
echo 'export PATH=$OLLAMA_DIR:$PATH' >> ~/.bashrc
source ~/.bashrc
