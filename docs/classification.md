# proc-lens v0.1 classification rules

The classifier is deterministic. Every matched rule produces an `Evidence` record containing a category, score, and human-readable reason. Scores are summed per category and the category with the highest score becomes the primary type.

When category scores tie, v0.1 uses this precedence:

```text
ROS2 > CONTAINER > SYSTEMD > DEV > BROWSER > PROCESS
```

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

A cgroup path containing a `.service` unit contributes **+80**.

The detail view preserves the full cgroup path and the table attempts to surface the service unit name.

## Development tools

Known development/build executable names contribute **+70**. v0.1 includes:

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

## Browsers

Process names containing one of the following contribute **+80**:

```text
firefox
chrome
chromium
brave
```

## Confidence

Confidence is derived from the winning category's accumulated score:

```text
high    >= 100
medium  60..99
low     < 60
```

A generic `PROCESS` has no category-specific evidence and therefore low confidence by definition.

## Deliberate limitations

The classifier does not attempt probabilistic or LLM-based inference in v0.1. It also does not query the ROS graph, Docker daemon, or systemd D-Bus API. Those integrations could improve semantic identity later, but they would increase dependencies and runtime coupling. The first release treats kernel-visible provenance as the source of truth.
