# Raspberry Pi 3B Setup & Deployment Guide

This guide explains how to cross-compile, deploy, and configure the **Tagatoni** recipe audit agent as a background daemon on a Raspberry Pi 3B running with an external SSD.

---

## 1. Prerequisites on the Raspberry Pi

Ensure your external SSD is connected and mounted to the Raspberry Pi. 

### Mounting your SSD (Optional)
If your SSD is not already mounted automatically, you can mount it to a fixed location (e.g. `/mnt/ssd`) by editing `/etc/fstab`:
1. Find your SSD UUID:
   ```bash
   sudo blkid
   ```
2. Create a mount directory:
   ```bash
   sudo mkdir -p /mnt/ssd
   ```
3. Edit `/etc/fstab` and append a line (replace with your UUID and filesystem type):
   ```text
   UUID=xxxx-xxxx-xxxx-xxxx  /mnt/ssd  ext4  defaults,noatime  0  2
   ```
4. Mount it:
   ```bash
   sudo mount -a
   ```

---

## 2. Compile on the Development Machine

To avoid overloading the Raspberry Pi's limited CPU and RAM (1 GB), always compile the binary on your development machine using one of the cross-compilers.

Navigate to your local `tagatoni` project folder and choose the target matching your Pi's OS:

### Option A: If your Raspberry Pi OS is 64-bit (Recommended)
```bash
# Using standard Cargo (requires local aarch64 gcc cross-compiler)
make build-rpi-64

# OR using Docker-based Cross (Recommended, no linkers required)
make build-rpi-cross-64
```
*Output binary will be at: `target/aarch64-unknown-linux-gnu/release/tagatoni`*

### Option B: If your Raspberry Pi OS is 32-bit (Legacy)
```bash
# Using standard Cargo (requires local armv7 gcc cross-compiler)
make build-rpi-32

# OR using Docker-based Cross
make build-rpi-cross-32
```
*Output binary will be at: `target/armv7-unknown-linux-gnueabihf/release/tagatoni`*

---

## 3. Deploy to the SSD

Once compiled, deploy the binary and configuration files directly to the SSD on the Pi using `scp`.

Assuming:
- Your Pi's hostname/IP is `pi@raspberrypi.local`
- Your SSD mount point is `/mnt/ssd`

### 1. Create directories on the SSD
```bash
ssh pi@raspberrypi.local "mkdir -p /mnt/ssd/tagatoni /mnt/ssd/tagatoni/logs"
```

### 2. Copy the binary
```bash
# (64-bit target example)
scp target/aarch64-unknown-linux-gnu/release/tagatoni pi@raspberrypi.local:/mnt/ssd/tagatoni/
```

### 3. Copy the configuration template and create your `.env`
```bash
scp .env.example pi@raspberrypi.local:/mnt/ssd/tagatoni/.env
```
Now, SSH into your Pi to customize your `.env` file with your credentials:
```bash
ssh pi@raspberrypi.local "nano /mnt/ssd/tagatoni/.env"
```

---

## 4. Run as a Systemd Service

To ensure the agent runs continuously in the background, restarts on failures, and boots automatically on system startup, configure it as a native `systemd` service.

### 1. Create the Service File
Create a new file on the Pi at `/etc/systemd/system/tagatoni.service`:
```bash
ssh pi@raspberrypi.local "sudo nano /etc/systemd/system/tagatoni.service"
```

Paste the following configuration (it configures the working directory directly on the SSD):
```ini
[Unit]
Description=Tagatoni Recipe Audit Agent
After=network-online.target mount.target
Wants=network-online.target

[Service]
Type=simple
User=pi
WorkingDirectory=/mnt/ssd/tagatoni
EnvironmentFile=/mnt/ssd/tagatoni/.env
ExecStart=/mnt/ssd/tagatoni/tagatoni
Restart=on-failure
RestartSec=15
StandardOutput=append:/mnt/ssd/tagatoni/logs/agent.log
StandardError=append:/mnt/ssd/tagatoni/logs/error.log

[Install]
WantedBy=multi-user.target
```

### 2. Manage the Service
Reload the systemd daemon, enable the service to start on boot, and spin it up:
```bash
# Reload systemd configuration
sudo systemctl daemon-reload

# Enable service auto-start
sudo systemctl enable tagatoni.service

# Start the agent
sudo systemctl start tagatoni.service
```

### 3. Check status & logs
```bash
# View active daemon status
sudo systemctl status tagatoni.service

# Stream agent logs
tail -f /mnt/ssd/tagatoni/logs/agent.log

# Stream error logs
tail -f /mnt/ssd/tagatoni/logs/error.log
```
