#!/bin/bash

# Update and install dependencies
sudo apt-get update
sudo apt-get install -y python3 python3-pip

# Install ollama
pip3 install ollama
