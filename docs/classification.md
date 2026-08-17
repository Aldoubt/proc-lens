# proc-lens v0.2.1 classification and provenance rules

proc-lens intentionally separates two concepts:

1. **Direct classification** — evidence that belongs to the current PID itself.
2. **Display provenance** — user-facing ownership inferred from the current PID's already-collected parent chain.

This distinction lets proc-lens say that an `Isolated Web Co` PID has no direct browser-name evidence of its own while still showing that it belongs to Firefox in the process table.

## Direct classification

The direct classifier is deterministic. Every matched rule produces an `Evidence` record containing a category, score, and human-readable reason.

Scores are summed per category. The highest accumulated score normally wins, and ties use this precedence:

```text
ROS2 > CONTAINER > SYSTEMD > DEV > BROWSER > PROCESS
```

ROS 2 has one additional rule: if any **strong ROS2 evidence** is present, ROS2 wins even when another category has a slightly higher accumulated score. Strong evidence is one of the +50-or-higher ROS2 rules below: an installed ROS2 executable path, `--ros-args`, a current `ros2 launch/run`, or an ancestor `ros2 launch/run`.

The environment-only rules (`ROS_VERSION=2` and `AMENT_PREFIX_PATH`) are intentionally **weak**. They remain score-based because Code, Firefox, terminals, and other applications can inherit these variables when launched from a shell that sourced a ROS workspace. Weak inherited environment alone therefore does not relabel a stronger direct `DEV` or `BROWSER` process as ROS2.

### ROS 2

| Evidence | Score | Strength |
| --- | ---: | --- |
| Resolvable `install/<package>/lib/<package>/<executable>` path in executable or command | +80 | strong |
| Command contains `--ros-args` | +70 | strong |
| Current command is `ros2 launch` or `ros2 run` | +60 | strong |
| Ancestor command is `ros2 launch` or `ros2 run` | +50 | strong |
| Selected environment contains `ROS_VERSION=2` | +40 | weak |
| Selected environment contains `AMENT_PREFIX_PATH` | +20 | weak |

The resolver checks both `/proc/<pid>/exe` and command arguments. This is intentional: an interpreted ROS 2 Python node may expose Python as the executable while its installed node path remains in `cmdline`.

Examples:

- `fast_livo --ros-args` under a concrete service remains direct `ROS2` because `--ros-args` is strong provenance.
- Generic `python3` with both ROS environment variables can still classify directly as `ROS2` with 60 accumulated points when no stronger competing category exists.
- `code` with the same inherited environment remains direct `DEV` because DEV has 70 points versus weak ROS2's 60.
- `firefox` with the same inherited environment remains direct `BROWSER` because BROWSER has 80 points versus weak ROS2's 60.

### Container

A cgroup path matching any of these markers contributes **+90**:

```text
/docker/
docker-
containerd
kubepods
libpod-
```

This covers common Docker/containerd/Kubernetes/Podman cgroup layouts without invoking their command-line tools.

### systemd

A **concrete** cgroup `*.service` unit contributes **+80**.

The per-user manager unit is explicitly ignored as SYSTEMD evidence:

```text
user@<numeric-uid>.service
```

This distinction matters on desktop Linux. Firefox, Code, terminals, and other user applications commonly inherit a path containing `user@1000.service`; that ancestry alone does not make them systemd services.

Concrete units such as these still qualify:

```text
todeskd.service
docker.service
camera-daemon.service
org.gnome.Shell@x11.service
```

A real GNOME user service can therefore correctly remain SYSTEMD. v0.2.1 does not force every desktop process to PROCESS.

### Development tools

Known development/build executable names contribute **+70**. v0.2.1 includes:

```text
code
code-insiders
clangd
rustc
cargo
cmake
ninja
make
colcon
gcc
g++
clang
clang++
cc1
cc1plus
rust-analyzer
pyright-langserver
pylsp
```

### Browsers

Process names containing one of the following contribute **+80**:

```text
firefox
chrome
chromium
brave
```

A browser under `user@<uid>.service` therefore remains direct `BROWSER` unless it has stronger competing provenance.

### Generic processes

A PID with no recognized category-specific evidence is direct `PROCESS`. Generic desktop applications do not inherit a SYSTEMD label solely from the user service manager.

### Confidence

Direct-classification confidence is derived from the selected category's accumulated score:

```text
high    >= 100
medium  60..99
low     < 60
```

A direct generic `PROCESS` has no category-specific evidence and therefore low confidence by definition.

## Display provenance

v0.2.1 adds a separate deterministic ownership resolver for the process table, CLI snapshot, filters, and search.

Display provenance never mutates the current PID's direct `Classification` or evidence.

### Inheritance eligibility

Only a PID whose direct classification is `PROCESS` may inherit a display category.

Direct categories are protected:

```text
ROS2
CONTAINER
SYSTEMD
DEV
BROWSER
```

Those categories keep their direct type even if a Browser or DEV ancestor exists.

### Nearest meaningful ancestor

For a direct PROCESS PID, walk the already-collected `parent_chain` from direct parent outward. The first ancestor whose **direct** type is one of these wins:

```text
BROWSER
DEV
```

Examples:

```text
firefox [direct BROWSER]
└── Isolated Web Co [direct PROCESS]
    -> display BROWSER, owner firefox, PROJECT Firefox
```

```text
firefox [direct BROWSER]
└── code [direct DEV]
    └── utility-process [direct PROCESS]
        -> nearest meaningful owner is code
        -> display DEV
```

The actual child PID/name/COMMAND remains visible. proc-lens does not merge process rows into one application row in v0.2.1.

### Browser-family PROJECT labels

Direct and inherited browser display rows use normalized application-family labels:

```text
firefox              -> Firefox
chrome/google-chrome -> Chrome
chromium             -> Chromium
brave                -> Brave
```

This is an ownership label, not a filesystem project.

### DEV project inference

For a direct or inherited DEV display row, project discovery checks cwd candidates in this order:

```text
current PID cwd
parent_chain[0] cwd
parent_chain[1] cwd
...
```

For each cwd candidate, proc-lens walks at most 8 filesystem parents and selects the first directory containing `.git`. The directory basename becomes PROJECT.

No `git` subprocess is spawned.

This allows a generic Code/Electron utility process whose own cwd is unrelated to still inherit a repository name from a meaningful ancestor when that ancestor is running inside the project.

If no Git root is found, PROJECT remains `-`.

## User-facing PROJECT precedence

After display provenance is resolved, PROJECT follows these semantics:

```text
ROS2      -> ROS2 workspace name
BROWSER   -> normalized browser family
DEV       -> nearest Git root from current/ancestor cwd chain
CONTAINER -> container
SYSTEMD   -> concrete service unit
PROCESS   -> -
```

The TUI limits the fixed-width PROJECT cell to 24 visible characters and uses Unicode `…` instead of hard clipping. Full values remain available in contexts that are not constrained to the table width.

## Filtering and search

The interactive TUI and CLI `snapshot --type ...` filter on the **display provenance type**, not only the direct classifier type.

Therefore a direct PROCESS child owned by Firefox appears under a Browser filter.

TUI search matches:

- raw process name
- raw command line
- resolved display type
- resolved PROJECT label

This makes searches such as `Firefox` useful even for child PIDs named `Isolated Web Co`.

## Inspect semantics

`proc-lens inspect <pid>` deliberately keeps the current PID's direct facts visible:

```text
Type       : PROCESS
Confidence : low
```

When ownership is inherited it adds a separate block:

```text
Provenance
Owner PID   : 7326
Owner       : firefox
Display type: BROWSER
Project     : Firefox
```

Executable, cwd, full command, cgroup, parent chain, ROS2 identity, and direct classification evidence remain raw/current-PID information.

## Deliberate limitations

v0.2.1 does not query the ROS graph, Docker daemon, systemd D-Bus, browser APIs, VS Code APIs, or an LLM. It does not collapse process families into one row. Parent-chain attribution remains deterministic and kernel-visible, which keeps the feature lightweight and explainable.