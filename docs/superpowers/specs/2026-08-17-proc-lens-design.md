# proc-lens v0.1 Design

## 1. Goal

`proc-lens` is a lightweight Linux terminal process inspector focused on explaining **what a process is, who started it, which project/workspace it belongs to, and how many CPU/RAM/GPU resources it consumes**.

The first release targets Ubuntu 22.04 / Linux x86_64 and is optimized for robotics development machines running ROS 2, Docker, NVIDIA workloads, compilers, IDEs, browsers, and normal desktop services.

The project intentionally does not attempt to replace `btop`, `htop`, `nvtop`, Prometheus, or full system observability platforms. Its differentiator is process provenance and project-aware classification.

## 2. Product Scope

### 2.1 v0.1 must provide

- Global CPU utilization
- Global memory utilization
- Optional NVIDIA GPU utilization and VRAM metrics
- Per-process PID, command, CPU, RAM, optional GPU/VRAM
- Process details: executable, command line, working directory, cgroup, parent chain
- Deterministic classification into at least:
  - ROS2
  - Docker/container
  - systemd service
  - Development tool
  - Browser
  - Generic process
- ROS2 workspace/package/executable resolution when detectable
- Flat process list and process-tree view
- Search/filter
- Sorting by CPU, RAM, and GPU
- Detail view with classification evidence
- Default 1 Hz refresh
- No root requirement for normal operation

### 2.2 Explicitly out of scope for v0.1

- Web dashboard
- Remote monitoring
- Prometheus exporter
- Container lifecycle management
- ROS graph/topic/service visualization
- Log aggregation
- LLM-based classification
- Long-term historical time-series storage
- GUI application
- Windows/macOS support
- Automatic process termination policies

## 3. Technology Choices

### 3.1 Language

Rust is used for:

- Low runtime overhead
- Single-binary distribution
- Strong typing around `/proc` parsing
- Safe concurrent sampling
- Good TUI ecosystem

### 3.2 Terminal UI

- `ratatui`
- `crossterm`

The UI is kept independent from collectors and classifiers so future alternative frontends do not require rewriting system inspection logic.

### 3.3 Linux process data

Prefer direct Linux interfaces:

- `/proc/stat`
- `/proc/meminfo`
- `/proc/<pid>/stat`
- `/proc/<pid>/status`
- `/proc/<pid>/cmdline`
- `/proc/<pid>/exe`
- `/proc/<pid>/cwd`
- `/proc/<pid>/cgroup`
- `/proc/<pid>/environ` when readable and needed

External commands such as `ps`, `pstree`, and `systemctl` are not part of the hot sampling loop.

### 3.4 NVIDIA GPU

Use NVIDIA NVML through a Rust wrapper with dynamic loading.

GPU support is optional. Failure to initialize NVML must degrade cleanly to CPU/RAM-only operation.

## 4. Architecture

```text
Linux /proc + optional NVML
          |
          v
      collectors
          |
          v
     ProcessSnapshot
          |
     +----+-------------+
     |                  |
     v                  v
 classifier        resolver
     |                  |
     +--------+---------+
              v
       EnrichedProcess
              |
              v
             app
              |
              v
             TUI
```

The primary rule is that collectors report facts, while classifiers and resolvers derive meaning.

## 5. Module Layout

```text
proc-lens/
├── Cargo.toml
├── README.md
├── LICENSE
├── src/
│   ├── main.rs
│   ├── app.rs
│   ├── collector/
│   │   ├── mod.rs
│   │   ├── cpu.rs
│   │   ├── memory.rs
│   │   ├── process.rs
│   │   └── gpu.rs
│   ├── process/
│   │   ├── mod.rs
│   │   ├── model.rs
│   │   ├── tree.rs
│   │   └── resolver.rs
│   ├── classifier/
│   │   ├── mod.rs
│   │   ├── rules.rs
│   │   ├── ros2.rs
│   │   ├── docker.rs
│   │   ├── systemd.rs
│   │   └── development.rs
│   ├── gpu/
│   │   ├── mod.rs
│   │   └── nvml.rs
│   └── ui/
│       ├── mod.rs
│       ├── dashboard.rs
│       ├── table.rs
│       ├── detail.rs
│       └── widgets.rs
└── tests/
    ├── classifier_tests.rs
    ├── resolver_tests.rs
    └── proc_parser_tests.rs
```

## 6. Core Data Model

Collectors produce immutable snapshots that contain raw facts. A simplified representation is:

```rust
pub struct ProcessSnapshot {
    pub pid: i32,
    pub ppid: i32,
    pub name: String,
    pub executable: Option<PathBuf>,
    pub cwd: Option<PathBuf>,
    pub command: Vec<String>,
    pub cgroup: Vec<String>,
    pub environment: LimitedEnvironment,
    pub cpu_percent: f32,
    pub memory_bytes: u64,
    pub gpu: Option<ProcessGpuUsage>,
}
```

Classification enriches this with:

```rust
pub struct EnrichedProcess {
    pub snapshot: ProcessSnapshot,
    pub process_type: ProcessType,
    pub project: Option<ProjectIdentity>,
    pub confidence: Confidence,
    pub evidence: Vec<Evidence>,
}
```

Classification evidence is a first-class field, not UI-only text.

## 7. CPU Sampling

Per-process CPU percentage is calculated from deltas between two samples:

- System total ticks from `/proc/stat`
- Process user/system ticks from `/proc/<pid>/stat`

The sampler stores the previous tick state keyed by `(pid, start_time)` rather than PID alone to avoid PID-reuse errors.

The UI refresh period defaults to 1 second. CPU collectors must avoid blocking the render thread.

## 8. Memory Sampling

Use process RSS from `/proc/<pid>/status` or `/proc/<pid>/statm`, normalized into bytes.

Global RAM is read from `/proc/meminfo` and uses an available-memory interpretation rather than simply `MemTotal - MemFree`.

## 9. Process Provenance

For each accessible process, resolve:

1. PID and PPID
2. Executable target
3. Full command line
4. Working directory
5. cgroup
6. Bounded parent chain
7. Selected environment keys only when necessary

The implementation must tolerate normal Linux races where a process exits between directory enumeration and reading one of its files.

Permission errors are represented as unavailable fields, not fatal errors.

## 10. Classification Engine

Classification is deterministic and evidence-based.

Each rule contributes evidence and a score. The winning category is selected from the strongest applicable category-specific evidence.

Example ROS2 evidence:

- executable path matches `*/install/<pkg>/lib/<pkg>/*`
- command line contains `--ros-args`
- ancestor command contains `ros2 launch` or `ros2 run`
- selected ROS/ament environment markers are present

Example container evidence:

- cgroup path contains Docker/containerd/container scope markers

Example systemd evidence:

- cgroup path maps to a `.service` unit under `system.slice` or user service scope

Example development evidence:

- known compiler/build/editor helper executables such as `clangd`, `rustc`, `cargo`, `cmake`, `ninja`, `make`, `colcon`, VS Code extension host processes

Rules must expose human-readable evidence such as:

```text
ROS2 / high confidence
- executable matches install/<pkg>/lib/<pkg>/<exe>
- command contains --ros-args
- ancestor contains ros2 launch
```

## 11. ROS2 Project Resolution

The resolver should derive project identity without requiring ROS2 APIs in v0.1.

Primary path-based heuristic:

```text
<workspace>/install/<package>/lib/<package>/<executable>
```

Expected output:

- workspace: directory before `install`
- package: package segment
- executable: final binary/script name

Additional evidence can come from cwd, ancestor launch processes, and environment variables.

Resolution is best-effort and must clearly distinguish `unknown` from confidently resolved values.

## 12. Process Tree

Build the process tree from sampled `pid -> ppid` relationships.

The tree view uses the same sampled process set as the flat view, avoiding a second system scan.

Cycles or missing parents caused by races are handled defensively.

## 13. GPU Collection

NVML initialization occurs once and remains optional.

Global GPU fields when supported:

- utilization
- used/total VRAM
- temperature
- power where inexpensive and supported

Per-process fields:

- GPU-associated PID
- VRAM usage when exposed by NVML
- per-process GPU utilization only where the installed NVML/driver exposes reliable accounting data

The UI must not fabricate a GPU percentage when only memory accounting is available. Unsupported values display as unavailable.

## 14. UI

### 14.1 Dashboard

```text
 proc-lens 0.1.0                           load 1.42 1.17 0.93
────────────────────────────────────────────────────────────────
CPU  ████████░░ 78%       RAM  ██████░░░░ 12.1 / 31.3G
GPU  █████░░░░░ 51%       VRAM ███░░░░░░░ 2.7 / 8.0G

PID     TYPE       PROJECT                 CPU    RAM     GPU  COMMAND
18452   ROS2       agt_navigation_runtime  83.4   1.21G     -  fast_livo
...

j/k move | Enter detail | / search | t tree | c cpu | m mem | g gpu | q quit
```

### 14.2 Detail view

Shows:

- PID/name/type/project
- CPU/RAM/GPU/VRAM
- executable
- cwd
- full command
- cgroup/systemd context
- parent chain
- ROS2 workspace/package/executable if available
- confidence and classification evidence

### 14.3 Interaction

Required keys:

- `j` / `k` or arrows: selection
- `Enter`: details
- `/`: search/filter
- `t`: flat/tree toggle
- `c`: CPU sort
- `m`: RAM sort
- `g`: GPU sort
- `q`: quit/back as appropriate

## 15. CLI

v0.1 exposes:

```text
proc-lens
proc-lens inspect <PID>
proc-lens snapshot
proc-lens --type <CATEGORY>
```

`snapshot` provides a non-interactive textual output suitable for bug reports and future scripting.

## 16. Error Handling

Normal, expected errors include:

- process exits during collection
- `/proc/<pid>/exe` unreadable
- `/proc/<pid>/environ` permission denied
- kernel thread has empty command line
- NVML absent
- GPU driver not loaded

These must never terminate the dashboard.

Fatal startup errors are limited to conditions where the terminal or required Linux process filesystem is unusable.

## 17. Performance Strategy

Performance targets for v0.1:

- idle CPU target: below 1% on a typical development workstation
- RSS target: below 50 MiB
- default refresh: 1 Hz

These are acceptance targets, not assumptions.

Optimizations:

- no shell commands in hot loop
- reuse buffers where practical
- cache static fields such as executable/cwd until process identity changes
- read environment only for unresolved processes that need additional evidence
- sample GPU independently and merge by PID
- render only after snapshot assembly

## 18. Testing

### Unit tests

- `/proc/stat` parsing
- `/proc/<pid>/stat` parsing including names containing spaces/parentheses
- status/memory parsing
- cgroup parsing
- process-tree construction
- classification scores/evidence
- ROS2 workspace/package resolution

### Fixture tests

Store synthetic `/proc`-like fixture text for deterministic parser and classifier tests.

### Integration tests

Spawn controlled child processes and verify:

- PID/PPID resolution
- command line
- cwd
- parent chain
- process exit races do not crash collection

GPU tests are optional/skipped when NVML is unavailable.

## 19. v0.1 Acceptance Criteria

The release is acceptable when:

- dashboard starts without root
- CPU/RAM global metrics update
- all readable processes are listed
- per-process CPU/RAM work across repeated samples
- process details expose provenance fields
- ROS2/Docker/systemd/dev/browser classification works on representative fixtures
- ROS2 install-layout resolver returns workspace/package/executable
- tree/search/sort/detail interactions work
- NVIDIA metrics appear when NVML is available and degrade safely otherwise
- parser/classifier/resolver tests pass
- measured idle CPU and RSS are documented

## 20. Implementation Order

1. Rust crate skeleton and core models
2. `/proc` parser and sampler with tests
3. process tree/provenance
4. deterministic classifier and ROS2 resolver
5. non-interactive `snapshot` and `inspect`
6. Ratatui dashboard and interactions
7. optional NVML integration
8. benchmarks, README, packaging, v0.1 polish

This order deliberately creates a usable CLI before the full TUI, making failures easier to diagnose and keeping the UI separate from system logic.
