# Runtime Smoke Tests

Run from the repository root:

```bash
./scripts/runtime-smoke.sh
./scripts/ros2-smoke.sh
```

Each run writes timestamped logs under `artifacts/runtime-smoke/`. The scripts
use the release binary at `./target/release/proc-lens`; the ROS2 script never
installs packages or changes system ROS configuration and cleans up its demo
nodes on exit.

Current acceptance boundary:

| Area | Result |
| --- | --- |
| Linux Runtime Foundation | PASS/FAIL |
| ROS Process classification | OBSERVE |
| ROS Node -> PID | IMPLEMENTED (conservative mapping) |
| ROS Topic topology | IMPLEMENTED (publisher/subscriber graph) |
| Topic Hz / bandwidth | NOT IMPLEMENTED |
| Dataflow findings/UI | NOT IMPLEMENTED |