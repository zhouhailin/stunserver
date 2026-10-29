# STUN Server - Deployment Makefile
# Purpose: Install, uninstall, and manage STUN server as a systemd service
# Usage: sudo make [target]
# Requires: root privileges

.PHONY: install uninstall start stop restart status help

INSTALL_DIR = /opt/stunserver
SERVICE_NAME = stunserver
SYSTEMD_DIR = /etc/systemd/system

# Install STUN server
install:
	@echo "Installing STUN server to $(INSTALL_DIR)..."
	@# Check root permissions
	@if [ "$$(id -u)" -ne 0 ]; then \
		echo "Error: root privileges required. Use sudo."; \
		exit 1; \
	fi
	@# Check systemd availability
	@if ! command -v systemctl >/dev/null 2>&1; then \
		echo "Error: systemd not found. This installer only supports systemd-based systems."; \
		echo "Please install systemd or use manual deployment."; \
		exit 1; \
	fi
	@# Create directory structure
	@mkdir -p $(INSTALL_DIR)/{bin,etc,log}
	@echo "Created directory structure: $(INSTALL_DIR)/{bin,etc,log}"
	@# Copy binary
	@if [ ! -f ../target/release/stunserver ]; then \
		echo "Error: Binary not found. Run 'make build' in project root first."; \
		exit 1; \
	fi
	@cp ../target/release/stunserver $(INSTALL_DIR)/bin/
	@chmod 755 $(INSTALL_DIR)/bin/stunserver
	@echo "Installed binary to $(INSTALL_DIR)/bin/stunserver"
	@# Copy configuration
	@if [ ! -f $(INSTALL_DIR)/etc/stunserver.conf ]; then \
		cp stunserver.conf.example $(INSTALL_DIR)/etc/stunserver.conf; \
		echo "Created configuration file: $(INSTALL_DIR)/etc/stunserver.conf"; \
	else \
		echo "Configuration file already exists, skipping"; \
	fi
	@# Copy systemd service
	@cp ../systemd/stunserver.service $(SYSTEMD_DIR)/$(SERVICE_NAME).service
	@echo "Installed systemd service"
	@# Reload systemd
	@systemctl daemon-reload
	@# Create system user if not exists
	@if ! id -u $(SERVICE_NAME) >/dev/null 2>&1; then \
		useradd -r -s /bin/false $(SERVICE_NAME); \
		echo "Created system user: $(SERVICE_NAME)"; \
	else \
		echo "System user $(SERVICE_NAME) already exists"; \
	fi
	@# Set ownership
	@chown -R $(SERVICE_NAME):$(SERVICE_NAME) $(INSTALL_DIR)
	@echo "Set ownership to $(SERVICE_NAME):$(SERVICE_NAME)"
	@# Enable service
	@systemctl enable $(SERVICE_NAME)
	@echo "Enabled systemd service"
	@echo ""
	@echo "Installation complete!"
	@echo "Next steps:"
	@echo "  sudo make start    - Start the service"
	@echo "  sudo make status   - Check service status"

# Uninstall STUN server
uninstall:
	@echo "Uninstalling STUN server..."
	@# Check root permissions
	@if [ "$$(id -u)" -ne 0 ]; then \
		echo "Error: root privileges required. Use sudo."; \
		exit 1; \
	fi
	@# Stop service if running
	@systemctl stop $(SERVICE_NAME) 2>/dev/null || true
	@echo "Stopped service"
	@# Disable service
	@systemctl disable $(SERVICE_NAME) 2>/dev/null || true
	@echo "Disabled service"
	@# Remove systemd service file
	@rm -f $(SYSTEMD_DIR)/$(SERVICE_NAME).service
	@systemctl daemon-reload
	@echo "Removed systemd service"
	@# Remove installation directory
	@if [ -d $(INSTALL_DIR) ]; then \
		rm -rf $(INSTALL_DIR); \
		echo "Removed $(INSTALL_DIR)"; \
	fi
	@# Remove system user (optional, comment out if you want to keep it)
	@if id -u $(SERVICE_NAME) >/dev/null 2>&1; then \
		userdel $(SERVICE_NAME) 2>/dev/null || true; \
		echo "Removed system user: $(SERVICE_NAME)"; \
	fi
	@echo ""
	@echo "Uninstallation complete!"

# Start service
start:
	@# Check root permissions
	@if [ "$$(id -u)" -ne 0 ]; then \
		echo "Error: root privileges required. Use sudo."; \
		exit 1; \
	fi
	@# Check if service is installed
	@if [ ! -f $(SYSTEMD_DIR)/$(SERVICE_NAME).service ]; then \
		echo "Error: Service not installed. Run 'sudo make install' first."; \
		exit 1; \
	fi
	@systemctl start $(SERVICE_NAME)
	@echo "Service started"
	@systemctl status $(SERVICE_NAME) --no-pager

# Stop service
stop:
	@# Check root permissions
	@if [ "$$(id -u)" -ne 0 ]; then \
		echo "Error: root privileges required. Use sudo."; \
		exit 1; \
	fi
	@systemctl stop $(SERVICE_NAME) 2>/dev/null || true
	@echo "Service stopped"

# Restart service
restart:
	@# Check root permissions
	@if [ "$$(id -u)" -ne 0 ]; then \
		echo "Error: root privileges required. Use sudo."; \
		exit 1; \
	fi
	@# Check if service is installed
	@if [ ! -f $(SYSTEMD_DIR)/$(SERVICE_NAME).service ]; then \
		echo "Error: Service not installed. Run 'sudo make install' first."; \
		exit 1; \
	fi
	@systemctl restart $(SERVICE_NAME)
	@echo "Service restarted"
	@systemctl status $(SERVICE_NAME) --no-pager

# Show service status
status:
	@# Check root permissions
	@if [ "$$(id -u)" -ne 0 ]; then \
		echo "Error: root privileges required. Use sudo."; \
		exit 1; \
	fi
	@systemctl status $(SERVICE_NAME) --no-pager

# Show help
help:
	@echo "STUN Server - Deployment Commands"
	@echo ""
	@echo "Available targets (requires root):"
	@echo "  sudo make install   - Install and enable service"
	@echo "  sudo make uninstall - Stop, disable, and remove service"
	@echo "  sudo make start     - Start service"
	@echo "  sudo make stop      - Stop service"
	@echo "  sudo make restart   - Restart service"
	@echo "  sudo make status    - Show service status"
	@echo "  sudo make help      - Show this help message"
	@echo ""
	@echo "For development commands, see ../Makefile"
