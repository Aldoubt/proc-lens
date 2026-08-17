# proc-lens v0.2 classification rules

The classifier is deterministic. Every matched rule produces an `Evidence` record containing a category, score, and human-readable reason.

Scores are summed per category. The highest accumulated score normally wins, and ties use this precedence:

```text
ROS2 > CONTAINER > SYSTEMD > DEV > BROWSER > PROCESS
```

ROS 2 has one additional provenance rule: if any **strong ROS2 evidence** is present, ROS2 wins even when another category has a slightly higher accumulated score. Strong evidence is one of the +50-or-higher ROS2 rules below: an installed ROS2 executable path, `--ros-args`, a current `ros2 launch/run`, or an ancestor `ros2 launch/run`.

The environment-only rules (`ROS_VERSION=2` and `AMENT_PREFIX_PATH`) are intentionally **weak**. They remain score-based because Code, Firefox, terminals, and other applications can inherit these variables when launched from a shell that sourced a ROS workspace. Weak inherited environment alone therefore does not relabel a stronger `DEV` or `BROWSER` process as ROS2.

## ROS 2

| Evidence | Score | Strength |
| --- | ---: | --- |
| Resolvable `install/<package>/lib/<package>/<executable>` path in executable or command | +80 | strong |
| Command contains `--ros-args` | +70 | strong |
| Current command is `ros2 launch` or `ros2 run` | +60 | strong |
| Ancestor command is `ros2 launch` or `ros2 run` | +50 | strong |
| Selected environment contains `ROS_VERSION=2` | +40 | weak |
| Selected environment contains `AMENT_PREFIX_PATH` | +20 | weak |

The project resolver checks both `/proc/<pid>/exe` and command arguments. This is intentional: an interpreted ROS 2 Python node may expose Python as the executable while its installed node path remains in `cmdline`.

Examples:

- `fast_livo --ros-args` under a concrete service remains `ROS2` because `--ros-args` is strong provenance.
- Generic `python3` with both ROS environment variables can still classify as `ROS2` with 60 accumulated points when no stronger competing category exists.
- `code` with the same inherited environment remains `DEV` because DEV has 70 points versus weak ROS2's 60.
- `firefox` with the same inherited environment remains `BROWSER` because BROWSER has 80 points versus weak ROS2's 60.

## Container

A cgroup path matching any of these markers contributes **+90**:

```text
/docker/
docker-
containerd
kubepods
libpod-
```

This covers common Docker/containerd/Kubernetes/Podman cgroup layouts without invoking their command-line tools.

## systemd

A **concrete** cgroup `*.service` unit contributes **+80**.

The per-user manager unit is explicitly ignored as SYSTEMD evidence:

```text
user@<numeric-uid>.service
```

This distinction matters on desktop Linux. Firefox, Code, GNOME Shell, terminals, and other user applications commonly inherit a path containing `user@1000.service`; that ancestry alone does not make them systemd services.

Concrete units such as these still qualify:

```text
todeskd.service
docker.service
camera-daemon.service
```

The same concrete-unit resolver is used for the PROJECT label, so an actual service displays its unit name rather than `user@1000.service`.

## Development tools

Known development/build executable names contribute **+70**. v0.2 includes:

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

For a `DEV` process, PROJECT walks upward from `cwd` for at most 8 parent transitions and uses the nearest directory containing `.git`. This is filesystem-only; proc-lens does not spawn `git`.

## Browsers

Process names containing one of the following contribute **+80**:

```text
firefox
chrome
chromium
brave
```

A browser under `user@<uid>.service` therefore remains `BROWSER` unless it has stronger competing provenance.

## Generic processes

A process with no recognized category-specific evidence is `PROCESS`. Generic desktop applications do not inherit a SYSTEMD label solely from the user service manager.

## Confidence

Confidence is derived from the selected category's accumulated score:

```text
high    >= 100
medium  60..99
low     < 60
```

A generic `PROCESS` has no category-specific evidence and therefore low confidence by definition.

## Project-label precedence

PROJECT is resolved independently from the category evidence display:

```text
ROS2 workspace
  > DEV nearest .git root
  > CONTAINER "container"
  > SYSTEMD concrete service unit
  > BROWSER / PROCESS "-"
```

## Deliberate limitations

The classifier does not attempt probabilistic or LLM-based inference in v0.2. It also does not query the ROS graph, Docker daemon, or systemd D-Bus API. Those integrations could improve semantic identity later, but they would increase dependencies and runtime coupling. v0.2 continues to treat kernel-visible provenance as the source of truth.
