#!/bin/sh
set -e

# Docker entrypoint script for STUN Server
# Handles environment variable overrides and configuration

CONFIG_PATH="${STUN_CONFIG_PATH:-/opt/stunserver/etc/stunserver.conf}"

# Function to update config file with environment variable
update_config() {
    local key=$1
    local value=$2
    local config_file=$3
    
    if [ -f "$config_file" ]; then
        # Check if key exists in config
        if grep -q "^${key}.*=" "$config_file"; then
            # Update existing key
            sed -i "s|^${key}.*=.*|${key} = ${value}|" "$config_file"
        else
            # Add new key (append to appropriate section)
            echo "${key} = ${value}" >> "$config_file"
        fi
    fi
}

# Apply environment variable overrides
if [ -n "$STUN_LOG_LEVEL" ]; then
    echo "Overriding log level with: $STUN_LOG_LEVEL"
    update_config "level" "\"$STUN_LOG_LEVEL\"" "$CONFIG_PATH"
fi

if [ -n "$STUN_PORT" ]; then
    echo "Overriding port with: $STUN_PORT"
    update_config "port" "$STUN_PORT" "$CONFIG_PATH"
fi

if [ -n "$STUN_LISTEN_ADDRESS" ]; then
    echo "Overriding listen address with: $STUN_LISTEN_ADDRESS"
    update_config "listen_address" "\"$STUN_LISTEN_ADDRESS\"" "$CONFIG_PATH"
fi

# Ensure log directory exists and has correct permissions
LOG_DIR="${STUN_LOG_DIR:-/opt/stunserver/log}"
if [ ! -d "$LOG_DIR" ]; then
    mkdir -p "$LOG_DIR"
fi

# Execute the main command
echo "Starting STUN server with config: $CONFIG_PATH"
exec "$@"
