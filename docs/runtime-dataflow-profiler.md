# proc-lens Runtime Dataflow Profiler

Status: design baseline

## 1. Goal

Evolve proc-lens from a PID-centric Linux process inspector into a low-overhead runtime observability tool for robotics and ROS 2 systems while preserving the existing process/provenance workflow.

The core question changes from only:

> What is this process and how many resources is it using?

into:

> Which runtime data path is consuming resources, where is latency accumulating, and which process/thread/topic is responsible?

proc-lens remains a fact collector and visualizer. Diagnosis and code-change reasoning belong to external consumers such as `agt_robot_agent`.

## 2. Repository boundary

### proc-lens owns

- Linux process/thread facts from `/proc` and related low-overhead kernel interfaces.
- Process provenance and project/workspace identity.
- ROS 2 graph awareness when ROS 2 is available.
- Runtime node/topic/process/thread relationships.
- Lightweight topic rate, message-size and throughput observations.
- Runtime budget evaluation from measured facts.
- Dataflow visualization and drill-down UI.
- Optional deep trace adapters.
- Machine-readable snapshots for external tools and AI agents.

### proc-lens does not own

- LLM reasoning.
- Automatic source-code modification.
- Robot navigation logic.
- Persistent robotics orchestration.
- Heavy tracing enabled by default.
- Full packet capture or point-cloud decoding in the normal sampling loop.

### agt_robot_agent owns

- Tool selection policy.
- Static repository audit.
- ARM/x86 portability reasoning.
- Runtime diagnosis from proc-lens evidence.
- Combining repository evidence with runtime evidence.
- Suggested fixes and optional code-edit workflows.

## 3. Two architectures

Software architecture answers:

- which package owns a capability;
- which module depends on which interface;
- where code belongs.

Runtime architecture answers:

- how many processes exist;
- which Linux threads execute callbacks;
- which CPU cores execute those threads;
- which ROS topics connect nodes;
- how much data is moving;
- where queueing, deadline misses, copies or contention appear.

proc-lens focuses on runtime architecture and provides evidence that can be correlated back to software architecture.

## 4. UI model

Keep the current Process View and add views incrementally.

### Process View

Existing PID-centric workflow:

- PID / PPID
- classification
- provenance
- project/workspace
- CPU / RAM / GPU / disk I/O

### Thread View

Per-process thread drill-down:

- TID
- thread name
- CPU utilization
- current/last observed CPU
- voluntary/involuntary context switches where available
- scheduler policy/priority where available
- CPU affinity where available

### ROS Graph View

ROS-aware topology:

- node
- namespace
- publishers/subscribers
- topic
- message type
- QoS summary
- mapped process when deterministically resolvable

Unknown process-to-node mappings must remain unknown rather than guessed.

### Dataflow View

Primary robotics view:

```text
MID360 driver
10 Hz / 5.1 MB/s
      |
      v
cloud filter
CPU 21% / p99 13 ms
      |
      +----> RViz
      |
      v
FAST-LIO2
CPU 158% / LiDAR 10 Hz / IMU 100 Hz
      |
      v
localization
0.5 Hz
      |
      v
Nav2 controller
50 Hz / jitter 1.8 ms
```

Node cards show compute facts. Edges show data-movement facts.

### Experiment View

Offline/replay comparison:

- run identifier
- git revision when supplied by caller
- rosbag identifier
- playback rate
- pass/fail
- CPU/RSS peaks
- topic rate stability
- callback/processing latency where instrumented
- deadline misses
- regression versus baseline

## 5. Observation levels

proc-lens must remain safe to leave running in the background.

### Level 0 — Static/runtime identity

Low-cost facts:

- process tree
- provenance
- workspace/repository
- executable/command line
- architecture and OS facts

### Level 1 — Normal

Default persistent mode:

- `/proc` process sampling
- `/proc/<pid>/task` thread inventory
- CPU/RAM/I/O
- ROS graph metadata at a conservative interval
- topic metadata and lightweight sampled throughput
- no heavy tracing
- no full PointCloud2 deserialization unless explicitly required

### Level 2 — Deep

Enabled for a selected node/path or experiment window:

- higher-rate thread observations
- selected topic rate/size sampling
- queue/backpressure indicators when observable
- process/thread scheduling details
- selected callback instrumentation when available

### Level 3 — Trace

Short diagnostic session only:

- ros2_tracing/LTTng adapters
- perf adapters
- scheduler/context-switch analysis
- callback start/end analysis

Trace mode must never become the default background mode.

## 6. Runtime data model

The UI and AI integrations consume normalized data instead of parsing terminal output.

### RuntimeSnapshot

```json
{
  "schema_version": 1,
  "timestamp_ns": 0,
  "host": {
    "arch": "x86_64",
    "logical_cpus": 16
  },
  "processes": [],
  "threads": [],
  "ros_nodes": [],
  "topics": [],
  "edges": [],
  "findings": []
}
```

### ProcessRecord

```json
{
  "pid": 4382,
  "start_time_ticks": 123456,
  "name": "fast_lio",
  "project": "agt_navigation_v3",
  "cpu_percent": 163.2,
  "rss_bytes": 1300234240,
  "thread_count": 11
}
```

### ThreadRecord

```json
{
  "pid": 4382,
  "tid": 4391,
  "name": "mapping",
  "cpu_percent": 62.0,
  "last_cpu": 4,
  "scheduler": "SCHED_OTHER"
}
```

### DataflowEdge

```json
{
  "transport": "ros2_topic",
  "name": "/livox/lidar",
  "source": "livox_driver",
  "target": "fast_lio",
  "rate_hz": 10.0,
  "mean_message_bytes": 510000,
  "bandwidth_bytes_s": 5100000,
  "drop_rate": null,
  "confidence": "measured"
}
```

### Finding

```json
{
  "severity": "warning",
  "type": "deadline_miss",
  "target": "controller_server",
  "evidence": {
    "expected_period_ms": 20.0,
    "observed_p99_period_ms": 31.0
  }
}
```

Unknown measurements use null/unknown semantics; proc-lens must not fabricate zeroes.

## 7. Budget-based warnings

Do not treat CPU percentage alone as the health signal.

Examples:

- A 50 Hz controller has a 20 ms period budget.
- A 10 Hz LiDAR path has a 100 ms arrival period.
- A process using 160% CPU on a multi-core host can be healthy.
- A controller using 8% CPU can still be unhealthy if its callback interval p99 is 31 ms.

Warnings should prefer deadline, backlog, rate loss and latency evidence over simplistic CPU thresholds.

## 8. Dataflow risk checks

The analyzer should eventually identify:

- publisher/subscriber fan-out on high-bandwidth topics;
- unexpectedly high aggregate data movement;
- redundant visualization/recording/debug subscribers;
- high-rate large messages;
- input rate greater than sustainable processing rate;
- queue growth/backpressure;
- output rate collapse while input rate remains stable;
- thread oversubscription;
- excessive CPU migration/context switching;
- runtime architecture that contradicts intended isolation/composition.

The tool must distinguish measured transport bytes from estimated aggregate movement.

## 9. ROS 2 mapping constraints

ROS node-to-PID mapping is not universally trivial. Use deterministic evidence first:

1. existing ROS-aware process identity/provenance;
2. command line and launch ancestry;
3. installed executable/package paths;
4. optional runtime instrumentation/registration.

If a composed process contains multiple ROS nodes, model one process containing multiple nodes rather than inventing one PID per node.

## 10. rosbag experiment contract

A repeatable experiment should support at minimum:

```text
0.5x -> correctness warm-up
1.0x -> real-time baseline
1.25x
1.5x
2.0x -> stress/headroom discovery
```

Each run records normalized runtime snapshots and a summary so runs can be compared without giving an AI raw logs.

Suggested run identity:

```text
experiment_id
host_fingerprint
repository_revision
bag_id
playback_rate
configuration_id
started_at
```

## 11. ARM64 and x86_64

Runtime collection must remain architecture-neutral where Linux interfaces are identical.

proc-lens should report host architecture and expose portability-relevant facts but should not attempt source-level portability diagnosis itself.

Near-term product work:

- validate Rust dependencies on aarch64;
- avoid x86-only assumptions in collectors;
- add aarch64 CI/source-build verification;
- add ARM64 release packaging only after runtime behavior is validated.

Source-code AVX/SSE/NEON/Eigen/PCL audits belong primarily to `agt_robot_agent` static analysis.

## 12. AI integration contract

proc-lens should expose deterministic structured evidence through CLI JSON first; daemon/API transport can follow later.

Candidate interfaces:

```text
proc-lens snapshot --json
proc-lens inspect <pid> --json
proc-lens threads <pid> --json
proc-lens ros graph --json
proc-lens dataflow --json
proc-lens dataflow inspect <node-or-topic> --json
proc-lens experiment report <run> --json
```

The exact command names are not frozen yet; the stable requirement is the schema, not CLI spelling.

`agt_robot_agent` should consume these outputs and decide when to escalate from Normal -> Deep -> Trace.

## 13. Version roadmap

### v0.3 — Runtime Model / ROS awareness foundation

Goal: build the normalized model without destabilizing the current TUI.

- thread inventory and basic thread metrics;
- host architecture/runtime facts;
- stable JSON snapshot schema;
- ROS graph collector behind an optional feature/runtime capability;
- deterministic process <-> ROS identity model where possible;
- initial dataflow graph model;
- tests for unknown/partial observations;
- x86_64 remains primary release target;
- begin aarch64 source-build validation.

Acceptance principle: existing process-inspection behavior and idle-overhead targets must not regress without measurement and documentation.

### v0.4 — Dataflow Profiler

Goal: make runtime bottlenecks visually obvious.

- Dataflow View;
- topic edges with Hz/message size/bandwidth;
- selected path drill-down;
- runtime budget configuration;
- warning/finding model;
- experiment/replay summaries;
- machine-readable interface for `agt_robot_agent`;
- optional Deep mode.

### Later

- trace adapters;
- callback-level visualization;
- historical comparisons;
- remote/daemon mode only if a concrete deployment requirement appears.

## 14. Design rules

1. Facts before inference.
2. Unknown is better than fabricated zero.
3. Keep Normal mode cheap enough for persistent use.
4. Node cards describe compute; edges describe data movement.
5. UI consumes normalized models; collectors never depend on UI.
6. AI consumes the same normalized models; AI logic does not live in proc-lens.
7. Heavy instrumentation is opt-in and targeted.
8. Preserve PID reuse protection using `(pid, start_time_ticks)` semantics.
9. Support composed ROS processes explicitly.
10. Benchmark every new collector on the target workstation before claiming overhead.
