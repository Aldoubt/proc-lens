# proc-lens

`proc-lens` is a lightweight Linux process inspector focused on one question that generic task managers do not answer well:

> **What is this process, who launched it, which application/project does it belong to, and what is it costing?**

It combines low-overhead `/proc` sampling with deterministic, explainable direct classification plus derived parent-chain provenance. v0.2.1 targets stable interactive use in Ubuntu/Linux robotics development environments with ROS 2, Docker/systemd, development tools, browsers, and optional NVIDIA GPU monitoring.

## What it shows

The layout and values below are illustrative; actual values come from the local Linux system at runtime.

```text
proc-lens 0.2.1      load 1.42 1.17 0.93      processes 241      mode cpu

CPU  [████████░░] 78.0%   RAM [██████░░░░] 12.1G / 31.3G   GPU0 [█████░░░░░] 51.0%

PID     TYPE       PROJECT                  CPU%       RAM    GPU%       VRAM  COMMAND
18452   ROS2       agt_navigation_runtime    83.4      1.21G      -     972.0M  fast_livo --ros-args
19221   DEV        proc-lens                  8.2      2.31G      2     120.0M  code --type=utility …
20422   BROWSER    Firefox                   14.8      4.18G      3     180.0M  firefox
20473   BROWSER    Firefox                    7.9      1.64G      -          -  Isolated Web Co
2137    SYSTEMD    todeskd.service            3.5      23.3M      -          -  ToDesk_Service

↑↓ move  Enter inspect  / search  Space pause  ? help
```

Selecting a process opens raw per-PID details—executable, working directory, full command line, cgroup, parent chain, ROS 2 identity, and direct classification evidence. When a generic child inherits application ownership, the detail view adds a separate `Provenance` section instead of pretending the inherited evidence belongs directly to that PID.

## Stable live view

v0.2 separates **sampling** from **ordering** so the table stays useful while processes are changing:

- system/process data is sampled every **1 second**
- per-process CPU shown in the TUI uses an EMA with `alpha = 0.35`
- CPU history is keyed by `(pid, start_time_ticks)`, so PID reuse does not inherit stale history
- automatic table reordering happens at most every **2 seconds**
- CPU sort uses **2 percentage-point bands**; rows inside the same band keep their previous relative order
- selection is anchored to the **PID**, not the row number
- search/filter changes preserve the previous visual position when the selected PID is filtered out
- `Space` freezes the whole view; while paused, `r` performs one refresh without resuming

CLI `snapshot` stays raw for resource values, while TYPE/PROJECT use the same deterministic provenance resolver as the TUI.

## Provenance refinement in v0.2.1

Direct classification and display provenance are intentionally separate:

- **direct classification** answers what this PID itself proves from executable/cmdline/environment/cgroup
- **display provenance** answers which meaningful parent application/project owns a generic child

Only a direct `PROCESS` child may inherit `BROWSER` or `DEV`, and only from the nearest meaningful ancestor in its existing parent chain. Direct ROS2, CONTAINER, SYSTEMD, DEV, and BROWSER rows are never overwritten.

Examples:

```text
firefox (BROWSER)
└── Isolated Web Co (direct PROCESS) -> display BROWSER / Firefox

code (DEV)
└── utility-process (direct PROCESS) -> display DEV / nearest Git project
```

This does not merge PIDs or hide subprocesses. The COMMAND column still shows the actual process identity; ownership is communicated through TYPE, PROJECT, and the detail provenance block.

## Design principles

- **Linux-native collection:** reads `/proc` directly; no `ps`, `pstree`, or `nvidia-smi` subprocesses in the sampling loop
- **Explainable classification:** direct ROS 2/container/systemd/development/browser evidence remains deterministic and inspectable
- **Derived provenance without extra daemons:** browser/DEV child ownership reuses the already-collected parent chain
- **Project-aware identity:** ROS 2 resolves workspace/package identity; DEV provenance checks current cwd and ancestor cwd values for the nearest `.git` root without spawning `git`
- **Stable selection:** selection is PID-based and reconciled after refresh/filter/reorder operations
- **Conservative GPU reporting:** missing per-process GPU utilization is rendered as `-`, never fabricated as `0%`
- **Graceful GPU fallback:** the rest of the application continues without NVML

## Requirements

- Linux (Ubuntu 22.04 / x86_64 is the first target)
- Rust 1.88 or newer
- optional NVIDIA driver exposing NVML for GPU metrics

## Build

```bash
cargo build --release
```

CPU-only build without the NVIDIA provider:

```bash
cargo build --release --no-default-features
```

The binary is created at `target/release/proc-lens`.

## Usage

```bash
proc-lens
proc-lens snapshot
proc-lens inspect 18452
proc-lens --type ros2
proc-lens --type dev snapshot
proc-lens --type browser snapshot
```

Accepted type names include `ros2`, `docker`/`container`, `systemd`, `dev`, `browser`, and `process`.

## Keyboard controls

| Key | Action |
| --- | --- |
| `j` / `↓` | Select next process |
| `k` / `↑` | Select previous process |
| `PageDown` / `PageUp` | Move one visible page |
| `Home` / `End` | Select first / last visible process |
| `Enter` | Open detail for the selected PID |
| `/` | Search command/type/project |
| `Space` | Pause or resume the live view |
| `r` | Refresh once while paused |
| `t` | Toggle process-tree order |
| `c` / `m` / `g` / `p` | Sort CPU / RAM / GPU / PID |
| `?` | Toggle help |
| `q` / `Esc` | Back, close, or quit |

## Classification

Direct classification is deterministic. Evidence scores are summed per category; the highest accumulated score normally wins, while ties use:

```text
ROS2 > CONTAINER > SYSTEMD > DEV > BROWSER > PROCESS
```

Strong ROS2 provenance—installed ROS2 node path, `--ros-args`, current `ros2 launch/run`, or an ancestor `ros2 launch/run`—is protected from competing categories. Environment-only evidence (`ROS_VERSION=2`, `AMENT_PREFIX_PATH`) stays score-based, so Code or Firefox launched from a ROS-sourced shell is not automatically relabeled ROS2.

`user@<uid>.service` is treated as the per-user service manager rather than proof that every desktop application is SYSTEMD. Concrete units such as `todeskd.service` or `org.gnome.Shell@x11.service` can correctly remain SYSTEMD.

Inherited Browser/DEV ownership is a separate display-provenance layer and never changes the current PID's direct evidence. See [`docs/classification.md`](docs/classification.md) for the complete rules.

## Project labels

The user-facing PROJECT label follows the resolved display provenance:

1. ROS 2 workspace name for ROS2 processes
2. normalized browser family (`Firefox`, `Chrome`, `Chromium`, `Brave`) for direct or inherited browser processes
3. for direct/inherited DEV, nearest `.git` root found from current cwd and then ancestor cwd values, each walking at most 8 filesystem parents
4. `CONTAINER` uses `container`
5. `SYSTEMD` displays the concrete service unit
6. unresolved generic `PROCESS` displays `-`

Fixed-width TUI PROJECT cells use a Unicode ellipsis `…` rather than hard clipping; full values remain available in detail/CLI contexts where width permits.

## CPU semantics

The top CPU gauge is **whole-system utilization**. Per-process CPU is a process-level metric and is not expected to sum directly to the top gauge, especially on multi-core machines. A process showing a larger numeric CPU percentage than the current whole-system gauge is therefore not automatically contradictory.

## GPU semantics

Global GPU utilization, memory, temperature, and power are read through NVML when available. Per-process VRAM is derived from active graphics/compute contexts. Per-process GPU utilization remains optional because the driver may not report a fresh utilization sample for every process on every refresh.

A missing process utilization value is rendered as `-`, not `0%`.

## Verification

Run the complete local verification chain:

```bash
./scripts/verify.sh
```

It checks formatting, Clippy, all-feature tests, CPU-only tests, and a release build. GitHub Actions runs the same quality gates plus an MSRV check against Rust 1.88.

Performance acceptance targets remain **<1% idle CPU** and **<50 MiB RSS** on a typical workstation. These are targets, not benchmark claims; measure them on the target machine before publishing numbers.

## Scope of v0.2.1

v0.2.1 deliberately does not implement process killing/renice, mouse control, grouped/collapsed process families, historical graphs, persistent configuration, web/remote monitoring, Prometheus export, Docker lifecycle management, ROS graph/topic visualization, log analysis, LLM-based classification, or cross-platform support.

## License

MIT
