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
| Bounded Topic Hz / message size / bandwidth | IMPLEMENTED (receive-side CLI observation) |
| Dataflow findings/UI | NOT IMPLEMENTED |

## Topic metric sampling boundary

`snapshot --json` samples at most two publisher-to-subscriber data topics by
default. Sampling uses a two-second receive-side observation window and skips
common infrastructure topics such as `/parameter_events`, `/rosout`, `/tf`,
`/tf_static`, and `/clock`. A populated metric record reports its source and
confidence explicitly. These observations are suitable for runtime profiling;
they are not publisher scheduling guarantees.
