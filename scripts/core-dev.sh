#!/usr/bin/env bash
set -euo pipefail

# Small temporary process manager for Amberite Core in dev.
# Run from repo root:  bash scripts/core-dev.sh {start|stop|restart|status|logs|wait}
#
# Designed for agent loops: restart will block until the HTTP port is open,
# print startup errors, and enforce a startup timeout so the caller never hangs.

# ---------------------------------------------------------------------------
# 1. Make cargo available in non-interactive shells (WSL, CI, etc.)
# ---------------------------------------------------------------------------
if ! command -v cargo &>/dev/null; then
	if [[ -f "$HOME/.cargo/env" ]]; then
		# shellcheck source=/dev/null
		source "$HOME/.cargo/env"
	fi
fi

# ---------------------------------------------------------------------------
# 2. Config & paths
# ---------------------------------------------------------------------------
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

CORE_DIR="$REPO_ROOT/apps/core"
LOG_DIR="$CORE_DIR/logs"
LOG_FILE="$LOG_DIR/core-dev.log"
PID_FILE="$CORE_DIR/.core-dev.pid"

HOST="${CORE_HOST:-127.0.0.1}"
PORT="${CORE_PORT:-16662}"
TIMEOUT="${CORE_DEV_TIMEOUT:-180}"
POLL_INTERVAL=1

# ---------------------------------------------------------------------------
# 3. Helpers
# ---------------------------------------------------------------------------
is_port_open() {
	# Bash built-in TCP probe — instant, no external tools needed.
	(echo >/dev/tcp/"$HOST"/"$PORT") &>/dev/null
}

pid_from_file() {
	[[ -f "$PID_FILE" ]] && cat "$PID_FILE" || true
}

is_pid_running() {
	local pid="$1"
	[[ -n "$pid" ]] && kill -0 "$pid" 2>/dev/null
}

# ---------------------------------------------------------------------------
# 4. Commands
# ---------------------------------------------------------------------------

show_logs() {
	if [[ -f "$LOG_FILE" ]]; then
		echo "--- last 50 lines of $LOG_FILE ---"
		tail -n 50 "$LOG_FILE"
		echo "------------------------------------"
	else
		echo "No log file found."
	fi
}

stop_core() {
	echo "[core-dev] Stopping core..."

	# 1) Kill the cargo wrapper we recorded.
	local pid
	pid=$(pid_from_file)
	if [[ -n "$pid" ]] && is_pid_running "$pid"; then
		kill "$pid" 2>/dev/null || true
		# Try its children too.
		pkill -P "$pid" 2>/dev/null || true
	fi

	# 2) Kill whoever is actually listening on the port (the real binary).
	local port_pid
	port_pid=""
	if command -v lsof &>/dev/null; then
		port_pid=$(lsof -ti tcp:"$PORT" 2>/dev/null || true)
	elif command -v fuser &>/dev/null; then
		port_pid=$(fuser "$PORT/tcp" 2>/dev/null || true)
	fi
	if [[ -n "$port_pid" ]]; then
		kill "$port_pid" 2>/dev/null || true
	fi

	rm -f "$PID_FILE"
	echo "[core-dev] Core stopped."
}

start_core() {
	local pid
	pid=$(pid_from_file)

	# If we already think it's running, sanity-check the port.
	if [[ -n "$pid" ]] && is_pid_running "$pid"; then
		if is_port_open; then
			echo "[core-dev] Core already online at http://$HOST:$PORT (PID: $pid)"
			return 0
		fi
		echo "[core-dev] Process exists (PID: $pid) but port not open yet — waiting..."
	else
		rm -f "$PID_FILE"
	fi

	# Also handle "orphan" server (port open but we lost the PID file).
	if is_port_open; then
		echo "[core-dev] Core already listening on port $PORT. Use 'restart' if you want a fresh instance."
		return 0
	fi

	mkdir -p "$LOG_DIR"
	# Truncate logs so agents always see fresh output on a new start.
	> "$LOG_FILE"

	echo "[core-dev] Starting core... timeout: ${TIMEOUT}s | log: $LOG_FILE"

	# Start cargo run in the background from the core directory.
	(
		cd "$CORE_DIR"
		cargo run >> "$LOG_FILE" 2>&1
	) &
	pid=$!
	echo "$pid" > "$PID_FILE"

	echo "[core-dev] Build/startup in progress (PID: $pid)..."

	local elapsed=0
	while [[ $elapsed -lt $TIMEOUT ]]; do
		if is_port_open; then
			echo "[core-dev] Core is ONLINE at http://$HOST:$PORT (PID: $pid)"
			return 0
		fi

		if ! is_pid_running "$pid"; then
			echo "[core-dev] ERROR: Core process exited before the port opened."
			show_logs
			rm -f "$PID_FILE"
			return 1
		fi

		sleep "$POLL_INTERVAL"
		elapsed=$((elapsed + POLL_INTERVAL))
	done

	echo "[core-dev] ERROR: Timed out after ${TIMEOUT}s waiting for core to come online."
	show_logs
	stop_core
	return 1
}

status_core() {
	local pid
	pid=$(pid_from_file)
	if [[ -n "$pid" ]] && is_pid_running "$pid"; then
		echo "[core-dev] Process running (PID: $pid)"
	else
		echo "[core-dev] Process not running"
	fi

	if is_port_open; then
		echo "[core-dev] Port $PORT is OPEN"
	else
		echo "[core-dev] Port $PORT is CLOSED"
	fi
}

wait_core() {
	echo "[core-dev] Waiting for core at $HOST:$PORT (timeout: ${TIMEOUT}s)..."
	local elapsed=0
	while [[ $elapsed -lt $TIMEOUT ]]; do
		if is_port_open; then
			echo "[core-dev] Core is online."
			return 0
		fi
		sleep "$POLL_INTERVAL"
		elapsed=$((elapsed + POLL_INTERVAL))
	done
	echo "[core-dev] ERROR: Timed out waiting for core."
	return 1
}

# ---------------------------------------------------------------------------
# 5. CLI dispatch
# ---------------------------------------------------------------------------
CMD="${1:-}"
case "$CMD" in
	start)
		start_core
		;;
	stop)
		stop_core
		;;
	restart)
		stop_core
		start_core
		;;
	status)
		status_core
		;;
	logs)
		show_logs
		;;
	wait)
		wait_core
		;;
	*)
		echo "Usage: $0 {start|stop|restart|status|logs|wait}"
		echo ""
		echo "Environment variables:"
		echo "  CORE_HOST          - Bind host to probe (default: 127.0.0.1)"
		echo "  CORE_PORT          - Port to probe (default: 16662)"
		echo "  CORE_DEV_TIMEOUT   - Startup timeout in seconds (default: 180)"
		exit 1
		;;
esac
