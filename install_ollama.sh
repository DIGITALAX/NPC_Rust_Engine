#!/bin/bash

# Definir directorio de instalación
INSTALL_DIR="/tmp/ollama"
OLLAMA_FILE="$INSTALL_DIR/ollama"

# Crear directorio de instalación
mkdir -p $INSTALL_DIR

# Descargar Ollama
echo "Descargando Ollama..."
curl -L https://ollama.com/download/ollama-linux-amd64 -o $OLLAMA_FILE
chmod +x $OLLAMA_FILE

# Verificar la instalación
if [ ! -f $OLLAMA_FILE ]; then
    echo "Error: No se pudo descargar Ollama"
    exit 1
fi

echo "Verificando la instalación de Ollama..."
if ! $OLLAMA_FILE --version &> /dev/null; then
    echo "Error: No se puede ejecutar Ollama"
    exit 1
fi

echo "Ollama se instaló correctamente en $OLLAMA_FILE"
echo "Contenido de $INSTALL_DIR:"
ls -l $INSTALL_DIR

# Agregar al PATH
echo "export PATH=$INSTALL_DIR:\$PATH" >> ~/.bashrc
echo "export OLLAMA_PATH=$OLLAMA_FILE" >> ~/.bashrc
source ~/.bashrc

echo "Instalación completada. Reinicia tu sesión o ejecuta 'source ~/.bashrc' para actualizar el PATH."