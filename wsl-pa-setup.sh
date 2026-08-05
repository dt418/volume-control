#!/usr/bin/env bash
# Install the PulseAudio daemon (WSL).
set -e
export DEBIAN_FRONTEND=noninteractive
exec > /tmp/vc-pa-setup.log 2>&1
apt-get update
apt-get install -y pulseaudio
ls -la /usr/bin/pulseaudio
echo PA_INSTALL_OK
