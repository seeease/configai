#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
cd "$SCRIPT_DIR"

PID_FILE="$SCRIPT_DIR/configai.pid"
LOG_DIR="$SCRIPT_DIR/logs/$(date +%Y%m)"
mkdir -p "$LOG_DIR"
LOG_FILE="$LOG_DIR/$(date +%d).configai.log"
BIN="$SCRIPT_DIR/target/release/configai"

# 构建
echo "Building configai..."
cargo build --release
echo "Build OK"

# 停止旧进程
if [ -f "$PID_FILE" ]; then
    OLD_PID=$(cat "$PID_FILE")
    if kill -0 "$OLD_PID" 2>/dev/null; then
        echo "Stopping old process (PID: $OLD_PID)..."
        kill "$OLD_PID"
        # 等待进程退出，最多 5 秒
        for i in $(seq 1 10); do
            kill -0 "$OLD_PID" 2>/dev/null || break
            sleep 0.5
        done
        # 还没退出就强杀
        if kill -0 "$OLD_PID" 2>/dev/null; then
            echo "Force killing $OLD_PID..."
            kill -9 "$OLD_PID" 2>/dev/null || true
        fi
        echo "Old process stopped"
    else
        echo "Old PID $OLD_PID not running, cleaning up"
    fi
    rm -f "$PID_FILE"
fi

# 启动新进程（后台运行）
echo "Starting configai..."
RUST_LOG=${RUST_LOG:-info} nohup "$BIN" serve "$@" >> "$LOG_FILE" 2>&1 &
NEW_PID=$!
echo "$NEW_PID" > "$PID_FILE"
echo "configai started (PID: $NEW_PID, log: $LOG_FILE)"
