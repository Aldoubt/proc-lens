# proc-lens v0.2.2 classification and provenance rules

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

### Container

A cgroup path matching `/docker/`, `docker-`, `containerd`, `kubepods`, or `libpod-` contributes **+90**.

### systemd

A concrete cgroup `*.service` unit contributes **+80**. The per-user manager `user@<numeric-uid>.service` is ignored as SYSTEMD evidence so desktop applications do not all become SYSTEMD. Concrete units such as `todeskd.service` and `org.gnome.Shell@x11.service` can correctly remain SYSTEMD.

### Development tools

Known development/build executable names contribute **+70**. This includes Code, clangd, Rust/Cargo tools, C/C++ build tools, colcon, rust-analyzer, pyright-langserver, and pylsp.

### Browsers

Process names containing Firefox/Chrome/Chromium/Brave markers contribute **+80**.

### Generic processes and confidence

A PID with no recognized category-specific evidence is direct `PROCESS`. Direct-classification confidence is derived from the winning category score:

```text
high    >= 100
medium  60..99
low     < 60
```

## Display provenance

Display provenance never mutates the current PID's direct `Classification` or evidence.

Only a PID whose direct classification is `PROCESS` may inherit a display category. For such a PID, proc-lens walks the already-collected `parent_chain` from direct parent outward; the nearest direct `BROWSER` or `DEV` ancestor wins.

Direct ROS2, CONTAINER, SYSTEMD, DEV, and BROWSER categories are protected and are never overwritten by an ancestor.

Example:

```text
firefox [direct BROWSER]
└── Isolated Web Co [direct PROCESS]
    -> display BROWSER, owner firefox, PROJECT Firefox
```

The child PID/name/COMMAND remains visible; proc-lens does not merge process rows.

### Browser-family PROJECT labels

Direct and inherited browser display rows use normalized application-family labels:

```text
firefox              -> Firefox
chrome/google-chrome -> Chrome
chromium             -> Chromium
brave                -> Brave
```

### DEV project inference and v0.2.2 family fallback

For a direct or inherited DEV display row, project discovery first checks cwd candidates in this order:

```text
current PID cwd
parent_chain[0] cwd
parent_chain[1] cwd
...
```

For each candidate, proc-lens walks at most 8 filesystem parents and selects the first directory containing `.git`. The directory basename becomes PROJECT. No `git` subprocess is spawned.

If no Git root is available, v0.2.2 applies a deliberately small application-family fallback:

```text
code / code-insiders        -> VS Code
rust-analyzer               -> Rust
clangd                      -> Clang
pyright-langserver / pylsp  -> Python
```

This fallback describes the **known development application/tool family**, not a repository. It exists specifically for cases such as VS Code GPU/zygote/utility processes whose cwd and ancestor cwd values do not identify one workspace and may in fact serve multiple windows.

The precedence is strict:

```text
verified Git root > approved DEV family > -
```

Therefore a `clangd` running inside `agt_navigation_v2` displays `agt_navigation_v2`, not `Clang`. A `code --type=gpu-process` with no Git evidence displays `VS Code`, not an arbitrarily guessed workspace. Unknown DEV tools with no Git evidence remain `-`.

## User-facing PROJECT precedence

After display provenance is resolved, PROJECT follows these semantics:

```text
ROS2      -> ROS2 workspace name
BROWSER   -> normalized browser family
DEV       -> nearest Git root; otherwise approved DEV family; otherwise -
CONTAINER -> container
SYSTEMD   -> concrete service unit
PROCESS   -> -
```

The TUI limits fixed-width PROJECT cells to 24 visible characters and uses Unicode `…` instead of hard clipping. Full values remain available where width permits.

## Filtering, search, and inspect

The TUI and CLI snapshot filters use the **display provenance type**, so a direct PROCESS child owned by Firefox appears under a Browser filter. Search matches raw process name, raw command line, resolved display type, and resolved PROJECT label.

`proc-lens inspect <pid>` keeps current-PID facts explicit. A browser child can therefore show:

```text
Type       : PROCESS
Confidence : low
Project    : Firefox

Provenance
Owner PID   : 7326
Owner       : firefox
Display type: BROWSER
Project     : Firefox
```

For a direct DEV process such as a VS Code GPU process, `Type : DEV` remains direct and `Project : VS Code` is the conservative family fallback; no inherited `Provenance` block is necessary unless ownership itself came from an ancestor.

## Deliberate limitations

v0.2.2 does not query the ROS graph, Docker daemon, systemd D-Bus, browser APIs, VS Code APIs/workspace databases, or an LLM. It does not collapse process families into one row. Parent-chain attribution and DEV fallback remain deterministic and lightweight, and proc-lens prefers `-` over inventing a project that cannot be supported by available evidence.
