#!/bin/bash
set -e

APP_NAME="system_manager_server"
INSTALL_DIR="/usr/local/bin"
CONFIG_DIR="/etc/system_manager_server"
DATA_DIR="/var/lib/system_manager_server"
CACHE_DIR="/var/cache/system_manager_server"
LOG_DIR="/var/log/system_manager_server"
PLUGIN_DIR="/usr/lib/system_manager_server"

echo "Installing $APP_NAME..."

# Check for required commands
for cmd in curl sudo; do
  if ! command -v $cmd &> /dev/null; then
    echo "Error: $cmd is not installed" >&2
    exit 1
  fi
done

# Create necessary directories
echo "Creating system directories..."
sudo mkdir -p "$CONFIG_DIR"
sudo mkdir -p "$DATA_DIR"
sudo mkdir -p "$CACHE_DIR"
sudo mkdir -p "$LOG_DIR"
sudo mkdir -p "$PLUGIN_DIR"

# ===This is for later===

# # Download the binary
# echo "Installing binary to $INSTALL_DIR..."
# sudo curl -L "https://yourdomain.com/releases/${APP_NAME}-latest" -o "$INSTALL_DIR/$APP_NAME"
# sudo chmod +x "$INSTALL_DIR/$APP_NAME"
#
# # Optionally install a default config file
# if [ ! -f "$CONFIG_DIR/config.toml" ]; then
#     echo "Installing default config..."
#     sudo curl -L "https://yourdomain.com/releases/config.toml" -o "$CONFIG_DIR/config.toml"
# fi
#
# ===This is for later===

echo "Installation of $APP_NAME complete!"
