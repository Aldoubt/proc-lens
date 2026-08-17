# proc-lens

`proc-lens` is a lightweight Linux process inspector focused on one question that generic task managers do not answer well:

> **What is this process, who launched it, which project does it belong to, and what is it costing?**

It combines low-overhead `/proc` sampling with deterministic, explainable process classification. v0.2 focuses on stable interactive use in Ubuntu/Linux robotics development environments with ROS 2, Docker/systemd, build tools, browsers, and optional NVIDIA GPU monitoring.

## What it shows

The layout and process values below are illustrative; actual values come from the local Linux system at runtime.

```text
proc-lens 0.2.0      load 1.42 1.17 0.93      processes 241      mode cpu

CPU  [████████░░] 78.0%   RAM [██████░░░░] 12.1G / 31.3G   GPU0 [█████░░░░░] 51.0%

PID     TYPE       PROJECT                  CPU%       RAM    GPU%       VRAM  COMMAND
18452   ROS2       agt_navigation_runtime    83.4      1.21G      -     972.0M  fast_livo --ros-args
18733   ROS2       agt_navigation_runtime    22.1      1.04G     21     950.0M  rviz2 --ros-args
19221   DEV        proc-lens                  8.2      2.31G      2     120.0M  code --type=utility …
19420   DEV        agt_navigation_v2         31.4      1.42G      -          -  clangd --background-index
20123   CONTAINER  container                  7.1     812.0M      -          -  python3 worker.py
20422   BROWSER    -                         14.8      4.18G      3     180.0M  firefox -contentproc …

↑↓ move  Enter inspect  / search  Space pause  ? help
```

Selecting a process opens provenance details including executable, working directory, full command line, cgroup, parent chain, ROS 2 workspace/package identity, and the evidence used by the classifier.

## Stable live view

v0.2 separates **sampling** from **ordering** so the table stays useful while processes are changing:

- System/process data is sampled every **1 second**.
- Per-process CPU shown in the TUI uses an EMA with `alpha = 0.35` to reduce one-sample spikes.
- CPU history is keyed by `(pid, start_time_ticks)`, so PID reuse does not inherit stale history.
- Automatic table reordering happens at most every **2 seconds**.
- CPU sort uses **2 percentage-point bands**; processes inside the same band keep their previous relative order.
- Selection is anchored to the **PID**, not the row number, so the highlight follows the same process when rows move.
- Search/filter changes preserve the previous visual position when the selected PID is filtered out, instead of jumping unconditionally to the first result.
- `Space` freezes the entire view. While paused, `r` performs one refresh without resuming. Press `Space` again to resume with an immediate refresh.

CLI `snapshot` and `inspect` stay raw and deterministic; smoothing is a TUI-only presentation feature.

## Design principles

- **Linux-native collection:** reads `/proc` directly; no `ps`, `pstree`, or `nvidia-smi` subprocesses in the sampling loop.
- **Explainable classification:** ROS 2, container, systemd, development, browser, or generic process categories are derived from visible rules and evidence.
- **Project-aware identity:** ROS 2 resolves workspace/package identity; development tools infer the nearest `.git` project root from `cwd` without spawning `git`.
- **Stable selection:** the interactive view owns selection by PID and reconciles it after refresh/filter/reorder operations.
- **PID reuse safety:** CPU presentation history is keyed by process identity `(pid, start_time_ticks)` rather than PID alone.
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
| `PageDown` / `PageUp` | Move one visible page |
| `Home` / `End` | Select first / last visible process |
| `Enter` | Open process detail for the selected PID |
| `/` | Search command/type/project |
| `Space` | Pause or resume the live view |
| `r` | Refresh once while paused |
| `t` | Toggle process-tree order |
| `c` | Sort by CPU |
| `m` | Sort by RAM |
| `g` | Sort by GPU utilization, then VRAM |
| `p` | Sort by PID |
| `?` | Toggle the in-app help overlay |
| `q` / `Esc` | Back, close overlay/search, or quit |

## Classification

Classification is deterministic. Evidence scores are summed per category; the highest accumulated score normally wins, while ties use this precedence:

```text
ROS2 > CONTAINER > SYSTEMD > DEV > BROWSER > PROCESS
```

Strong ROS2 provenance is protected from competing categories: an installed ROS2 node path, `--ros-args`, current `ros2 launch/run`, or an ancestor `ros2 launch/run` makes ROS2 the primary type. Environment-only evidence (`ROS_VERSION=2` and `AMENT_PREFIX_PATH`) remains score-based, so launching Code or Firefox from a ROS-sourced shell does not incorrectly relabel them as ROS2.

A process detail view shows the evidence and accumulated score for its selected category, for example:

```text
Type       : ROS2
Confidence : high

Classification evidence
- [ROS2 +80] executable matches install/<package>/lib/<package>/<executable>
- [ROS2 +70] command contains --ros-args
- [ROS2 +40] environment contains ROS_VERSION=2
```

v0.2 treats `user@<uid>.service` as the per-user service manager rather than evidence that every desktop application is a SYSTEMD process. Concrete units such as `todeskd.service` remain SYSTEMD evidence. This allows Firefox to classify as `BROWSER`, development tools such as Code/clangd as `DEV`, and ordinary desktop processes as `PROCESS` when no stronger evidence exists.

See [`docs/classification.md`](docs/classification.md) for the complete v0.2 rules.

## Project labels

The PROJECT column is resolved in this order:

1. ROS 2 workspace name from the installed node path.
2. For `DEV`, the nearest `.git` ancestor of `cwd`, walking at most 8 parents.
3. `CONTAINER` uses `container` in v0.2.
4. `SYSTEMD` displays the concrete service unit.
5. `BROWSER` and generic `PROCESS` display `-`.

The main table intentionally shows a compact command label for scanability. `Enter` detail and `proc-lens inspect <pid>` retain the complete command line and provenance.

## GPU semantics

Global GPU utilization, memory, temperature, and power are read through NVML when available. Per-process VRAM is derived from active graphics/compute process contexts. Per-process GPU utilization is kept optional because the underlying driver API may not report a fresh utilization sample for every process on every refresh.

A missing process utilization value is rendered as `-`, not `0%`.

## Verification

Run the complete local verification chain:

```bash
./scripts/verify.sh
```

It checks formatting, Clippy, all-feature tests, CPU-only tests, and a release build. GitHub Actions runs the same quality gates plus an MSRV check against Rust 1.88.

Performance acceptance targets remain **<1% idle CPU** and **<50 MiB RSS** on a typical workstation. These are targets, not hard-coded claims; record measurements on the target machine before publishing benchmark numbers.

## Scope of v0.2

v0.2 deliberately does not implement process killing/renice, mouse control, historical graphs, persistent configuration, web/remote monitoring, Prometheus export, Docker lifecycle management, ROS graph/topic visualization, log analysis, LLM-based classification, or cross-platform support. The focus remains process provenance and a stable local inspection workflow.

## License

MIT
