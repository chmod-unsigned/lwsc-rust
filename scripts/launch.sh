#!/bin/bash

modem_behaviour() {
	sudo sysctl -w net.ipv4.tcp_window_scaling=0
	sudo sysctl -w net.ipv6.conf.all.disable_ipv6=1
	sudo sysctl -w net.ipv6.conf.default.disable_ipv6=1
}

fiber_behaviour() {
	sudo sysctl -w net.ipv4.tcp_window_scaling=1
	sudo sysctl -w net.ipv6.conf.all.disable_ipv6=0
	sudo sysctl -w net.ipv6.conf.default.disable_ipv6=0
}

cleanup() {
	fiber_behaviour
	if [ -n "$BG_PID" ] && kill -0 "$BG_PID" 2>/dev/null; then
		kill "$BG_PID" 2>/dev/nul
	fi

	exit 130
}

trap cleanup INT TERM

bots=("lastwar-alexception" "lastwar-notsostaui" "lastwar-vp")

modem_behaviour

for bot in "${bots[@]}"; do
	./venv/bin/python3 autoretry.py &

	# Snapshot existing LastWar PIDs BEFORE launching this bot
	KNOWN_PIDS=$(pgrep 'LastWar.exe' 2>/dev/null || true)

	echo "Launch: $bot"
	lutris lutris:rungame/$bot &

	# Wait for a NEW LastWar.exe PID that wasn't in the snapshot
	PID=""
	while [ -z "$PID" ]; do
		for p in $(pgrep 'LastWar.exe' 2>/dev/null); do
			if ! echo "$KNOWN_PIDS" | grep -qw "$p"; then
				PID="$p"
				break
			fi
		done
		sleep 1
	done
	echo "pid: $PID ($bot)"

	# Wait for the X11 window to appear for this PID, then rename it
	WID=""
	while [ -z "$WID" ]; do
		WID=$(xdotool search --onlyvisible --pid "$PID" 2>/dev/null | head -1)
		sleep 1
	done
	xdotool set_window --name "lwsc2-$bot" "$WID"
	echo "Renamed window $WID for $bot (pid: $PID)"
done
