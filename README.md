# proc-lens

`proc-lens` is a lightweight Linux process inspector focused on one question that generic task managers do not answer well:

> **What is this process, who launched it, which project does it belong to, and what is it costing?**

It combines low-overhead `/proc` sampling with deterministic, explainable process classification. The first release is optimized for Ubuntu/Linux robotics development environments with ROS 2, Docker/systemd, build tools, browsers, and optional NVIDIA GPU monitoring.

## What it shows

The layout and process values below are illustrative; actual values come from the local Linux system at runtime.

```text
proc-lens 0.1.0      load 1.42 1.17 0.93      processes 241      mode cpu

CPU  [████████░░] 78.0%   RAM [██████░░░░] 12.1G / 31.3G   GPU0 [█████░░░░░] 51.0%

PID     TYPE       PROJECT                  CPU%       RAM    GPU%       VRAM  COMMAND
18452   ROS2       agt_navigation_runtime    83.4      1.21G      -     972.0M  fast_livo --ros-args ...
18733   ROS2       agt_navigation_runtime    22.1      1.04G     21     950.0M  rviz2 --ros-args ...
19221   DEV        code                       8.2      2.31G      2     120.0M  /usr/share/code/code ...
19420   DEV        clangd                    31.4      1.42G      -          -  clangd ...
20123   CONTAINER  container                  7.1     812.0M      -          -  python3 ...
20422   BROWSER    firefox                   14.8      4.18G      3     180.0M  firefox ...

j/k move | Enter detail | / search | t tree | c cpu | m mem | g gpu | q quit
```

Selecting a process opens provenance details including executable, working directory, command line, cgroup, parent chain, ROS 2 workspace/package identity, and the evidence used by the classifier.

## Design principles

- **Linux-native collection:** reads `/proc` directly; no `ps`, `pstree`, or `nvidia-smi` subprocesses in the sampling loop.
- **Explainable classification:** ROS 2, container, systemd, development, browser, or generic process categories are derived from visible rules and evidence.
- **Project-aware ROS 2 resolution:** resolves `install/<pkg>/lib/<pkg>/<exe>` from both `/proc/<pid>/exe` and command paths, which also covers interpreted Python nodes.
- **PID reuse safety:** CPU history is keyed by `(pid, start_time_ticks)` rather than PID alone.
- **Conservative GPU reporting:** per-process GPU utilization is shown only when the driver reports it; VRAM can still be shown independently.
- **Graceful GPU fallback:** NVIDIA support is optional at runtime; the rest of the application continues without NVML.

## Requirements

- Linux (Ubuntu 22.04 / x86_64 is the first target)
- Rust 1.88 or newer
- Optional: NVIDIA driver exposing NVML for GPU metrics

## Build

```bash
cargo build --release
```

CPU-only build without the NVIDIA provider:

```bash
cargo build --release --no-default-features
```

The binary is created at:

```text
target/release/proc-lens
```

## Usage

Launch the interactive dashboard:

```bash
proc-lens
```

Print one snapshot:

```bash
proc-lens snapshot
```

Explain a PID:

```bash
proc-lens inspect 18452
```

Filter the dashboard or snapshot by category:

```bash
proc-lens --type ros2
proc-lens --type dev snapshot
proc-lens --type docker snapshot
```

Accepted type names include `ros2`, `docker`/`container`, `systemd`, `dev`, `browser`, and `process`.

## Keyboard controls

| Key | Action |
| --- | --- |
| `j` / `↓` | Select next process |
| `k` / `↑` | Select previous process |
| `Enter` | Open process detail |
| `q` / `Esc` | Back or quit |
| `/` | Search command/type/project |
| `t` | Toggle process-tree order |
| `c` | Sort by CPU |
| `m` | Sort by RAM |
| `g` | Sort by GPU utilization, then VRAM |
| `p` | Sort by PID |

## Classification

Classification is deterministic. A process detail view shows the evidence that produced the result, for example:

```text
Type       : ROS2
Confidence : high

Classification evidence
- [ROS2 +80] executable matches install/<package>/lib/<package>/<executable>
- [ROS2 +70] command contains --ros-args
- [ROS2 +40] environment contains ROS_VERSION=2
```

See [`docs/classification.md`](docs/classification.md) for the complete v0.1 rule set.

## GPU semantics

Global GPU utilization, memory, temperature, and power are read through NVML when available. Per-process VRAM is derived from active graphics/compute process contexts. Per-process GPU utilization is kept optional because the underlying driver API may not report a fresh utilization sample for every process on every refresh.

A missing process utilization value is rendered as `-`, not `0%`.

## Verification

Run the complete local verification chain:

```bash
./scripts/verify.sh
```

It checks formatting, Clippy, all-feature tests, CPU-only tests, and a release build. GitHub Actions runs the same quality gates plus an MSRV check.

Performance acceptance targets for v0.1 are **<1% idle CPU** and **<50 MiB RSS** at the default 1 Hz refresh on a typical workstation. These are targets, not hard-coded claims; record measurements on the target machine before publishing benchmark numbers.

## Scope of v0.1

The first release deliberately does not implement a web dashboard, remote monitoring, Prometheus export, Docker lifecycle management, ROS graph/topic visualization, log analysis, LLM-based classification, or cross-platform support. The focus is process provenance and fast local inspection.

## License

MIT
