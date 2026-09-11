#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
cd "$ROOT_DIR"
timestamp=$(date -u +%Y%m%d-%H%M%S)
artifact_dir="$ROOT_DIR/artifacts/runtime-smoke/$timestamp-ros2"
mkdir -p "$artifact_dir"
proc_lens="$ROOT_DIR/target/release/proc-lens"
talker_pid=""
listener_pid=""

cleanup() {
    trap - EXIT INT TERM
    for pid in "$talker_pid" "$listener_pid"; do
        if [[ -n "$pid" ]] && kill -0 "$pid" 2>/dev/null; then
            kill -- "-$pid" 2>/dev/null || kill "$pid" 2>/dev/null || true
        fi
    done
    for pid in "$talker_pid" "$listener_pid"; do
        if [[ -n "$pid" ]]; then
            wait "$pid" 2>/dev/null || true
        fi
    done
}
trap cleanup EXIT INT TERM

if [[ ! -x "$proc_lens" ]]; then
    echo "FAIL: missing $proc_lens; run ./scripts/runtime-smoke.sh first" | tee "$artifact_dir/summary.txt"
    exit 1
fi

if [[ ! -f /opt/ros/humble/setup.bash ]]; then
    echo "SKIP ROS demo: /opt/ros/humble/setup.bash is missing" | tee "$artifact_dir/summary.txt"
    exit 0
fi
set +u
source /opt/ros/humble/setup.bash
set -u

if ! command -v ros2 > "$artifact_dir/ros2-command.txt" 2>&1; then
    echo "SKIP ROS demo: ros2 is not available after sourcing Humble" | tee "$artifact_dir/summary.txt"
    exit 0
fi
ros2 --help > "$artifact_dir/ros2-help.txt" 2>&1
if ! ros2 pkg prefix demo_nodes_cpp > "$artifact_dir/demo-nodes-prefix.txt" 2>&1; then
    echo "SKIP ROS demo: demo_nodes_cpp is not installed; no apt install attempted" | tee "$artifact_dir/summary.txt"
    exit 0
fi

setsid ros2 run demo_nodes_cpp talker > "$artifact_dir/talker.log" 2>&1 &
talker_pid=$!
setsid ros2 run demo_nodes_cpp listener > "$artifact_dir/listener.log" 2>&1 &
listener_pid=$!
printf 'talker_pid=%s\nlistener_pid=%s\n' "$talker_pid" "$listener_pid" > "$artifact_dir/node-pids.txt"

graph_ready=false
for _ in {1..10}; do
    if ros2 node list 2> "$artifact_dir/ros2-node-list.err" | tee "$artifact_dir/ros-nodes.txt" | grep -qE '/(talker|listener)$'; then
        graph_ready=true
        break
    fi
    sleep 1
done
if [[ "$graph_ready" != true ]]; then
    echo "WARNING: ROS graph did not show talker/listener within 10 seconds" | tee "$artifact_dir/graph-warning.txt"
fi

ros2 node list > "$artifact_dir/ros-nodes.txt" 2>&1 || true
ros2 topic list > "$artifact_dir/ros-topics.txt" 2>&1 || true
timeout 5s ros2 topic hz /chatter > "$artifact_dir/ros-topic-hz.txt" 2>&1 || true
"$proc_lens" --type ros2 snapshot > "$artifact_dir/ros-snapshot.txt"
"$proc_lens" snapshot --json > "$artifact_dir/runtime.json"

python3 scripts/check-runtime-json.py "$artifact_dir/runtime.json" \
    --report-json "$artifact_dir/report.json" \
    --report-text "$artifact_dir/summary.txt"
python3 - "$artifact_dir/runtime.json" "$artifact_dir/processes.txt" "$artifact_dir/threads.txt" "$artifact_dir/summary.txt" <<'PY'
import json
import sys

runtime_path, processes_path, threads_path, summary_path = sys.argv[1:]
runtime = json.load(open(runtime_path))
with open(processes_path, "w") as output:
    for process in runtime["processes"]:
        output.write(f"{process['pid']}\t{process['process_type']}\t{process['name']}\n")
with open(threads_path, "w") as output:
    for thread in runtime["threads"]:
        output.write(f"{thread['pid']}\t{thread['tid']}\t{thread['name']}\t{thread.get('scheduler')}\n")
with open(summary_path, "a") as output:
    output.write("\nROS2 smoke comparison:\n")
    output.write("  ROS graph nodes: see ros-nodes.txt\n")
    output.write("  Linux PID/process snapshot: see processes.txt\n")
    output.write("  proc-lens ROS2 classification: see ros-snapshot.txt and processes.txt\n")
    output.write("  topics/edges: topology implemented; bounded Hz/size/bandwidth metrics enabled\n")
    output.write("  ROS node -> PID mapping: see ROS node mappings below\n")
PY
python3 - "$artifact_dir/runtime.json" "$artifact_dir/summary.txt" <<'PY'
import json
import sys

runtime_path, summary_path = sys.argv[1:]
runtime = json.load(open(runtime_path))
nodes = {node["full_name"]: node for node in runtime["ros_nodes"]}
missing = sorted({"/talker", "/listener"} - nodes.keys())
if missing:
    raise SystemExit("missing expected ROS graph nodes: " + ", ".join(missing))

identities = {}
with open(summary_path, "a") as output:
    output.write("\nROS node mappings:\n")
    for name in ("/talker", "/listener"):
        node = nodes[name]
        identity = node.get("process_identity")
        if not identity or identity.get("pid", 0) <= 0 or identity.get("start_time_ticks", 0) <= 0:
            raise SystemExit(f"invalid process identity for {name}: {identity!r}")
        identities[name] = (identity["pid"], identity["start_time_ticks"])
        output.write(
            f"  {name}: pid={identity['pid']} start_time_ticks={identity['start_time_ticks']} "
            f"confidence={node['mapping_confidence']} source={node['mapping_source']}\n"
        )
if identities["/talker"][0] == identities["/listener"][0]:
    raise SystemExit("talker and listener unexpectedly share a PID")
PY
python3 - "$artifact_dir/runtime.json" "$artifact_dir/summary.txt" <<'PY'
import json
import sys

runtime_path, summary_path = sys.argv[1:]
runtime = json.load(open(runtime_path))
topics = {topic["name"]: topic for topic in runtime["topics"]}
chatter = topics.get("/chatter")
if chatter is None:
    raise SystemExit("missing expected /chatter topic topology")
if "/talker" not in chatter.get("publishers", []):
    raise SystemExit("/talker is not recorded as a /chatter publisher")
if "/listener" not in chatter.get("subscribers", []):
    raise SystemExit("/listener is not recorded as a /chatter subscriber")

edge = next(
    (
        item
        for item in runtime["edges"]
        if item.get("topic") == "/chatter"
        and item.get("publisher_node") == "/talker"
        and item.get("subscriber_node") == "/listener"
    ),
    None,
)
if edge is None:
    raise SystemExit("missing expected /talker -> /chatter -> /listener edge")

with open(summary_path, "a") as output:
    output.write("\nROS topic topology:\n")
    output.write(
        f"  /chatter: publishers={chatter['publishers']} "
        f"subscribers={chatter['subscribers']} types={chatter['message_types']}\n"
    )
    output.write("  edge: /talker -> /chatter -> /listener PASS\n")

metrics = chatter.get("metrics")
if not metrics:
    raise SystemExit("missing bounded metrics for /chatter")
for field in ("receive_frequency_hz", "mean_message_bytes", "receive_bandwidth_bytes_per_sec"):
    value = metrics.get(field)
    if not isinstance(value, (int, float)) or value <= 0:
        raise SystemExit(f"invalid /chatter metric {field}: {value!r}")
if metrics.get("confidence") not in {"estimated", "partial"}:
    raise SystemExit(f"unexpected /chatter metric confidence: {metrics.get('confidence')!r}")

with open(summary_path, "a") as output:
    output.write(
        "  metrics: "
        f"hz={metrics['receive_frequency_hz']} "
        f"mean_bytes={metrics['mean_message_bytes']} "
        f"bandwidth_Bps={metrics['receive_bandwidth_bytes_per_sec']} "
        f"confidence={metrics['confidence']} PASS\n"
    )
PY
echo "PASS: proc-lens and ROS2 smoke commands completed" | tee -a "$artifact_dir/summary.txt"