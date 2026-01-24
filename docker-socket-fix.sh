#!/bin/bash
# Docker Socket Fix for macOS

echo "=== Docker Socket Fix Script ==="
echo "Attempting to establish Docker connection..."

# Kill any existing Docker processes
echo "1. Stopping existing Docker processes..."
pkill -f "Docker Desktop" 2>/dev/null
pkill -f "com.docker" 2>/dev/null
sleep 2

# Start Docker Desktop
echo "2. Starting Docker Desktop..."
open -a Docker

# Wait for Docker to initialize
echo "3. Waiting for Docker to initialize (30 seconds)..."
for i in {1..30}; do
    echo -n "."
    sleep 1
done
echo ""

# Check Docker status
echo "4. Checking Docker status..."
docker version 2>&1

# Try to create socket link if needed
echo "5. Attempting to create socket link..."
if [ ! -e /var/run/docker.sock ]; then
    echo "Socket not found at /var/run/docker.sock"
    
    # Find any available docker.sock
    SOCK=$(find /Users/$(whoami) /var -name "docker.sock" 2>/dev/null | head -1)
    
    if [ -n "$SOCK" ]; then
        echo "Found socket at: $SOCK"
        echo "Creating symlink (requires sudo)..."
        sudo ln -sf "$SOCK" /var/run/docker.sock
    else
        echo "No docker.sock found. Docker may still be initializing."
    fi
else
    echo "Socket exists at /var/run/docker.sock"
fi

# Final check
echo "6. Final Docker check..."
docker version