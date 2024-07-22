#!/bin/bash

INSTALL_DIR="/tmp/ollama"
OLLAMA_FILE="$INSTALL_DIR/ollama"

mkdir -p $INSTALL_DIR

echo "Descargando Ollama..."
curl -L https://ollama.com/download/ollama-linux-amd64 -o $OLLAMA_FILE
chmod +x $OLLAMA_FILE

if [ ! -f $OLLAMA_FILE ]; then
    echo "Error: No se pudo descargar Ollama"
    exit 1
fi

echo "Verificando la instalación de Ollama..."
if ! $OLLAMA_FILE --version &> /dev/null; then
    echo "Error: No se puede ejecutar Ollama"
    exit 1
fi

echo "ollama installed successfully in $OLLAMA_FILE"

export PATH=$OLLAMA_DIR:$PATH
if ! grep -q "$OLLAMA_DIR" ~/.bashrc; then
    echo "export PATH=$OLLAMA_DIR:\$PATH" >> ~/.bashrc
fi

echo "ollama contents $OLLAMA_FILE"
ls -l $OLLAMA_DIR

source ~/.bashrc

echo "Installation completed. PATH updated for the current session."