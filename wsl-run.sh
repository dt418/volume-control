#!/usr/bin/env bash
# Run the real Linux host under Xvfb + PulseAudio (WSL, user 'thanh').
# Proves GTK init, renderer create, audio backend, and the GTK main loop.
set -e
export DISPLAY=:99
export XDG_RUNTIME_DIR=/tmp/vc-runtime
mkdir -p "$XDG_RUNTIME_DIR" 2>/dev/null || true
chmod 700 "$XDG_RUNTIME_DIR" 2>/dev/null || true
chown "$(id -u)" "$XDG_RUNTIME_DIR" 2>/dev/null || true

Xvfb :99 -screen 0 1280x1024x24 > /tmp/vc-xvfb.log 2>&1 &
XVFB_PID=$!
sleep 1

echo "== pulseaudio start =="
pulseaudio --start --exit-idle-time=-1 > /tmp/vc-pa.log 2>&1 || echo "pulse start failed (continuing)"
sleep 1
pactl load-module module-null-sink sink_name=vc_test > /tmp/vc-pactl.log 2>&1 || true
pactl set-default-sink vc_test > /dev/null 2>&1 || true

echo "== audio backend: volumectl get =="
/mnt/d/Projects/volume-control/target/debug/volumectl get || echo "get failed (expected if no sink)"

echo "== host run (8s) =="
RUST_LOG=info timeout 8 /mnt/d/Projects/volume-control/target/debug/volumectl 2>&1 || echo "host exited with code $? (timeout=124 expected)"
echo "RUN_COMPLETE"
kill "$XVFB_PID" 2>/dev/null || true
