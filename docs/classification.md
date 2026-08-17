# proc-lens v0.2 classification rules

The classifier is deterministic. Every matched rule produces an `Evidence` record containing a category, score, and human-readable reason.

v0.2 applies category precedence whenever more than one category has positive evidence:

```text
ROS2 > CONTAINER > SYSTEMD > DEV > BROWSER > PROCESS
```

Scores are still summed inside each category and are used for the selected category's confidence. They are not used to let a lower-priority category override a higher-priority category. For example, valid ROS 2 evidence remains ROS2 even when the process also runs under a concrete systemd service.

## ROS 2

| Evidence | Score |
| --- | ---: |
| Resolvable `install/<package>/lib/<package>/<executable>` path in executable or command | +80 |
| Command contains `--ros-args` | +70 |
| Current command is `ros2 launch` or `ros2 run` | +60 |
| Ancestor command is `ros2 launch` or `ros2 run` | +50 |
| Selected environment contains `ROS_VERSION=2` | +40 |
| Selected environment contains `AMENT_PREFIX_PATH` | +20 |

The project resolver checks both `/proc/<pid>/exe` and command arguments. This is intentional: an interpreted ROS 2 Python node may expose Python as the executable while its installed node path remains in `cmdline`.

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

A browser under `user@<uid>.service` therefore remains `BROWSER` unless it has evidence for a higher-priority category.

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
