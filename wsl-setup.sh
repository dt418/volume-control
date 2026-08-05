#!/usr/bin/env bash
# Linux host verification setup for volume-control (WSL Ubuntu 24.04).
set -e
export DEBIAN_FRONTEND=noninteractive
exec > /tmp/vc-setup.log 2>&1
echo "== apt update =="
apt-get update
echo "== apt install =="
apt-get install -y --no-install-recommends curl build-essential pkg-config \
  libpulse-dev libgtk-4-dev libadwaita-1-dev \
  libayatana-appindicator3-dev libx11-dev libxkbcommon-dev \
  xvfb dbus-x11
echo "== rustup =="
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain stable --profile minimal
echo "== rustc/cargo =="
export PATH="$HOME/.cargo/bin:$PATH"
rustc --version
cargo --version
echo "SETUP_DONE"
