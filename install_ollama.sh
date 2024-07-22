#!/bin/bash

PROJECT_DIR="$HOME/project/src"
OLLAMA_DIR="$PROJECT_DIR"
OLLAMA_FILE="$OLLAMA_DIR/ollama"
mkdir -p $OLLAMA_DIR

echo "Downloading ollama..."
curl -L https://ollama.com/download/ollama-linux-amd64 -o $OLLAMA_FILE
chmod +x $OLLAMA_FILE

if [ ! -f $OLLAMA_FILE ]; then
    echo "ollama could not be downloaded"
    exit 1
fi

if ! $OLLAMA_FILE --version &> /dev/null; then
    echo "ollama could not be executed"
    exit 1
fi

echo "ollama installed successfully in $OLLAMA_FILE"

export PATH=$OLLAMA_DIR:$PATH
BASHRC_FILE="$HOME/.bashrc"

if [ ! -f $BASHRC_FILE ]; then
    touch $BASHRC_FILE
fi

if ! grep -q "$OLLAMA_DIR" $BASHRC_FILE; then
    echo "export PATH=$OLLAMA_DIR:\$PATH" >> $BASHRC_FILE
fi

echo "ollama contents $OLLAMA_FILE"
ls -l $OLLAMA_DIR

source $BASHRC_FILE

echo "Installation completed. PATH updated for the current session."
