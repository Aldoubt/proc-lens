# proc-lens

[English](#english) · [中文](#中文)

`proc-lens` is a lightweight Linux process inspector for understanding **what a process is, who launched it, which application/project it belongs to, and what resources it is consuming**.

`proc-lens` 是一个轻量级 Linux 进程检查工具，重点回答：**这个进程是什么、由谁启动、属于哪个应用/项目，以及正在消耗多少资源**。

<p align="center">
  <img src="docs/assets/proc-lens-v0.2.2-tui.png" alt="proc-lens TUI" width="900">
</p>

> Screenshot captured from a real Ubuntu workstation. Values are machine/runtime dependent.  
> 截图来自真实 Ubuntu 工作站；资源数值会随机器和运行状态变化。

---

# English

## Why proc-lens?

Tools such as `top`, `htop`, and `btop` are excellent at showing resource usage, but on a robotics/development workstation the harder question is often **provenance**:

- Is this PID a ROS 2 node, a browser child, a systemd service, or a development tool?
- Which ROS 2 workspace or Git project does it belong to?
- Is `Isolated Web Co` actually part of Firefox?
- Is a `code` subprocess tied to a repository, or only to the VS Code application family?
- Can the answer remain deterministic and inspectable rather than being guessed?

`proc-lens` combines direct `/proc` sampling with deterministic classification and a separate parent-chain provenance layer.

## v0.2.x highlights

- **Linux-native collection** from `/proc`; no `ps`, `pstree`, or `nvidia-smi` subprocesses in the sampling loop
- **Stable TUI** with 1 Hz sampling, CPU EMA smoothing, delayed reordering, PID-anchored selection, search, pause, and manual refresh
- **Explainable direct classification**: `ROS2`, `CONTAINER`, `SYSTEMD`, `DEV`, `BROWSER`, `PROCESS`
- **Parent-chain provenance** for generic Browser/DEV children without changing their raw per-PID evidence
- **ROS 2-aware identity** using installed-node paths, `--ros-args`, launch/run ancestry, and workspace/package resolution
- **Concrete systemd unit detection** while ignoring `user@<uid>.service` as generic desktop-service-manager evidence
- **Git-aware DEV project labels** from current/ancestor cwd values
- **Conservative DEV family fallback** when no repository can be proven
- **Optional NVIDIA NVML support** for global GPU state and per-process VRAM where available
- Missing per-process GPU utilization is shown as `-`, never fabricated as `0%`

## Ubuntu package installation — v0.2.3

v0.2.3 adds Ubuntu packaging without changing the runtime/TUI behavior. Ubuntu 22.04+ x86_64 users can download the `.deb` attached to the `v0.2.3` GitHub Release and install it directly. Rust and Cargo are **not required at runtime**.

```bash
sudo apt install ./proc-lens_0.2.3-1_amd64.deb
```

After installation, either run:

```bash
proc-lens
```

or open the Ubuntu application menu, search for **proc-lens**, and click the icon. The installed desktop entry uses:

```ini
Exec=proc-lens
Icon=proc-lens
Terminal=true
```

so the desktop launches the existing TUI in a terminal instead of introducing a GUI wrapper or hard-coding GNOME Terminal.

Uninstall:

```bash
sudo apt remove proc-lens
```

Developers can build and validate the `.deb` locally:

```bash
cargo install cargo-deb --locked --version 3.7.0
./scripts/build-deb.sh
```

Expected output:

```text
target/debian/proc-lens_0.2.3-1_amd64.deb
```

`build-deb.sh` runs the normal project verification, builds with `cargo-deb`, and validates package name/version/architecture plus the installed binary, desktop entry, SVG/PNG icons, README, and LICENSE with `dpkg-deb`.

## Stable live view

Sampling and ordering are intentionally separated so the table remains usable while CPU usage changes:

- process/system sampling: **1 second**
- TUI process CPU smoothing: EMA with `alpha = 0.35`
- CPU history key: `(pid, start_time_ticks)` to avoid PID-reuse contamination
- automatic reordering: at most every **2 seconds**
- CPU sorting: **2 percentage-point bands** to reduce row thrashing
- selection: anchored to the **PID**, not the current row number
- `Space`: pause/resume the complete view
- `r`: perform one refresh while remaining paused

CLI `snapshot` keeps raw resource values, while TYPE/PROJECT use the same deterministic provenance resolver as the TUI.

## Direct classification vs display provenance

These are intentionally separate concepts.

**Direct classification** answers what the current PID itself proves from executable, command line, selected environment, and cgroup evidence.

**Display provenance** answers which meaningful parent application/project owns a generic child.

Example:

```text
firefox [direct BROWSER]
└── Isolated Web Co [direct PROCESS]
    -> display BROWSER / Firefox
```

`inspect` therefore keeps the raw fact:

```text
Type       : PROCESS
Confidence : low
```

and adds ownership separately:

```text
Provenance
Owner PID   : 7326
Owner       : firefox
Display type: BROWSER
Project     : Firefox
```

This keeps provenance useful without pretending inherited evidence belongs directly to the child PID.

## DEV project semantics

For a direct or inherited DEV process, PROJECT follows this precedence:

```text
verified Git workspace
    ↓ unavailable
known development application family
    ↓ unavailable
-
```

Git discovery checks the current cwd and ancestor cwd values, walking at most 8 filesystem parents for a `.git` root. No `git` subprocess is spawned.

The fallback map is deliberately small:

```text
code / code-insiders        -> VS Code
rust-analyzer               -> Rust
clangd                      -> Clang
pyright-langserver / pylsp  -> Python
```

A family label is **not a guessed repository**. A VS Code GPU process serving multiple windows can therefore show `PROJECT = VS Code` instead of being incorrectly assigned to one arbitrary repository. A verified Git repository always wins over the family fallback.

## Classification notes

Direct evidence scores are accumulated by category; ties use:

```text
ROS2 > CONTAINER > SYSTEMD > DEV > BROWSER > PROCESS
```

Strong ROS 2 evidence—installed ROS 2 node path, `--ros-args`, current `ros2 launch/run`, or a `ros2 launch/run` ancestor—is protected from competing categories. Environment-only evidence such as `ROS_VERSION=2` and `AMENT_PREFIX_PATH` remains weak/score-based so Firefox or VS Code launched from a ROS-sourced shell is not automatically relabeled ROS2.

For systemd, `user@<uid>.service` is treated as the per-user service manager rather than proof that every desktop process is SYSTEMD. Concrete units such as `todeskd.service`, `pulseaudio.service`, or `org.gnome.Shell@x11.service` can still correctly classify as SYSTEMD.

See [`docs/classification.md`](docs/classification.md) for the complete rules.

## Requirements

For source builds:

- Linux; Ubuntu 22.04 / x86_64 is the first target
- Rust **1.88 or newer**
- optional NVIDIA driver exposing NVML for GPU metrics

For `.deb` users, Rust/Cargo is not required.

## Build from source

```bash
cargo build --release
```

CPU-only build without the NVIDIA provider:

```bash
cargo build --release --no-default-features
```

Binary:

```text
target/release/proc-lens
```

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
| `Enter` | Inspect selected PID |
| `/` | Search command/type/project |
| `Space` | Pause / resume |
| `r` | Refresh once while paused |
| `t` | Toggle process-tree order |
| `c` / `m` / `g` / `p` | Sort by CPU / RAM / GPU / PID |
| `?` | Toggle help |
| `q` / `Esc` | Back / close / quit |

## CPU and GPU semantics

The top CPU gauge is **whole-system utilization**. Per-process CPU is a process-level metric and is not expected to sum directly to the top gauge on a multi-core machine.

Global GPU utilization, memory, temperature, and power are read through NVML when available. Per-process VRAM is derived from active graphics/compute contexts. Per-process GPU utilization is optional because a driver may not report a fresh utilization sample for every PID; missing values remain `-`.

## Verification

Run the complete local verification chain:

```bash
./scripts/verify.sh
```

It checks formatting, Clippy, all-feature tests, CPU-only tests, and a release build. GitHub Actions adds an MSRV check on Rust 1.88 plus a real Ubuntu `.deb` build/validation job.

Performance goals remain **<1% idle CPU** and **<50 MiB RSS** on a typical workstation. These are acceptance targets, not published benchmark claims; measure them on the target machine before quoting numbers.

## Scope of v0.2.3

v0.2.3 is **packaging-only**. It adds cargo-deb metadata, an Ubuntu desktop launcher, SVG/PNG icons, deterministic `.deb` validation, and tag-triggered GitHub Release publishing. It intentionally does **not** add process killing/renice, mouse control, grouped process-family views, historical graphs, persistent configuration, web/remote monitoring, Prometheus export, Docker lifecycle management, ROS graph/topic visualization, VS Code workspace-database inspection, log analysis, LLM classification, AppImage/Snap/Flatpak, ARM64 release artifacts, GUI wrappers, or new runtime classification behavior.

---

# 中文

## 为什么做 proc-lens？

`top`、`htop`、`btop` 已经非常擅长展示资源占用，但在机器人和开发工作站上，更难回答的往往不是“哪个 PID 占 CPU”，而是**这个 PID 到底属于谁**：

- 它是 ROS 2 节点、浏览器子进程、systemd 服务，还是开发工具？
- 它属于哪个 ROS 2 workspace 或 Git 项目？
- `Isolated Web Co` 实际上是不是 Firefox 的子进程？
- 某个 `code` 子进程能否确定属于具体仓库，还是只能确定属于 VS Code？
- 在没有可靠证据时，工具能不能明确保持“不知道”，而不是为了填满界面去猜？

`proc-lens` 的核心就是把低开销 `/proc` 采样、可解释的直接分类和独立的父进程 provenance 归属层组合起来。

## v0.2.x 主要特性

- **Linux 原生采集**：直接读取 `/proc`，采样循环不调用 `ps`、`pstree` 或 `nvidia-smi`
- **稳定 TUI**：1 Hz 采样、CPU EMA 平滑、低频重排、PID 锚定选择、搜索、暂停和单次刷新
- **可解释直接分类**：`ROS2`、`CONTAINER`、`SYSTEMD`、`DEV`、`BROWSER`、`PROCESS`
- **父链 Provenance**：为 Generic 浏览器/开发子进程推断上层归属，但不修改当前 PID 的原始证据
- **ROS 2 感知**：识别安装路径、`--ros-args`、`ros2 launch/run` 父链，并解析 workspace/package
- **具体 systemd service 识别**：忽略 `user@<uid>.service` 这种用户 service manager 的泛化干扰
- **Git 感知 DEV PROJECT**：从当前进程和父链 cwd 中寻找可验证仓库
- **保守 DEV 应用族回退**：无法证明具体仓库时只显示应用族，不伪造项目名
- **可选 NVIDIA NVML**：提供全局 GPU 信息和可获得的进程 VRAM
- 无法得到进程 GPU 利用率时显示 `-`，不会伪造为 `0%`

## Ubuntu 软件包安装 — v0.2.3

v0.2.3 只增加 Ubuntu 打包和发布链路，不改变现有 TUI/运行时行为。Ubuntu 22.04+ x86_64 用户可以下载 `v0.2.3` GitHub Release 中的 `.deb` 直接安装，运行时**不需要 Rust 或 Cargo**。

```bash
sudo apt install ./proc-lens_0.2.3-1_amd64.deb
```

安装后既可以执行：

```bash
proc-lens
```

也可以打开 Ubuntu 应用菜单，搜索 **proc-lens** 并点击图标。安装的桌面启动器使用：

```ini
Exec=proc-lens
Icon=proc-lens
Terminal=true
```

因此系统会自动打开终端进入现有 TUI，不增加 GUI wrapper，也不绑定 GNOME Terminal。

卸载：

```bash
sudo apt remove proc-lens
```

开发者本地构建和校验 `.deb`：

```bash
cargo install cargo-deb --locked --version 3.7.0
./scripts/build-deb.sh
```

预期生成：

```text
target/debian/proc-lens_0.2.3-1_amd64.deb
```

`build-deb.sh` 会先运行原有质量门，再调用 `cargo-deb`，随后用 `dpkg-deb` 验证软件包名称/版本/架构，以及二进制、desktop entry、SVG/PNG 图标、README 和 LICENSE 是否真正进入包内。

## 稳定实时视图

v0.2.x 将“采样”和“排序”分离，避免 CPU 轻微变化导致列表频繁跳动：

- 系统/进程采样：**1 秒**
- TUI 进程 CPU：EMA 平滑，`alpha = 0.35`
- CPU 历史按 `(pid, start_time_ticks)` 保存，避免 PID 复用继承旧数据
- 自动重排：最多约 **2 秒一次**
- CPU 排序：使用 **2 个百分点分档**，同档进程尽量保持相对顺序
- 选中状态：跟随 **PID**，而不是“第几行”
- `Space`：冻结/恢复完整视图
- 暂停时 `r`：只刷新一次，不自动恢复

CLI `snapshot` 的资源数值保持原始采样值，TYPE/PROJECT 则和 TUI 共用同一套确定性的 provenance resolver。

## 直接分类与 Display Provenance

这两层语义刻意分开。

**Direct Classification（直接分类）**回答：当前 PID 自己的 executable、cmdline、环境变量和 cgroup 能证明什么。

**Display Provenance（展示归属）**回答：一个 Generic 子进程属于哪个有意义的上层应用/项目。

例如：

```text
firefox [直接 BROWSER]
└── Isolated Web Co [直接 PROCESS]
    -> 展示 BROWSER / Firefox
```

因此 `inspect` 仍然会保留：

```text
Type       : PROCESS
Confidence : low
```

同时单独增加：

```text
Provenance
Owner PID   : 7326
Owner       : firefox
Display type: BROWSER
Project     : Firefox
```

这样既能让主表好读，也不会把父进程证据假装成当前 PID 自己的证据。

## DEV PROJECT 的语义

直接或继承得到的 DEV 进程按以下优先级生成 PROJECT：

```text
可验证的 Git workspace
    ↓ 没有
已知开发应用族
    ↓ 没有
-
```

Git 项目推断会依次检查当前 cwd 和父链 cwd，每个 cwd 最多向上检查 8 层寻找 `.git`，不会启动额外 `git` 子进程。

当前只加入少量保守映射：

```text
code / code-insiders        -> VS Code
rust-analyzer               -> Rust
clangd                      -> Clang
pyright-langserver / pylsp  -> Python
```

这里的应用族**不是猜测出来的仓库名**。例如一个同时服务多个 VS Code 窗口的 GPU process，cwd 只有 `/home/user` 时，可以显示 `PROJECT = VS Code`，但不会被强行归到某个任意仓库。只要存在真实 Git workspace，真实仓库名始终优先于 `VS Code`、`Rust`、`Clang` 或 `Python` 这类回退标签。

## 分类规则要点

直接证据按类别累计分数，通常由最高分胜出；同分优先级为：

```text
ROS2 > CONTAINER > SYSTEMD > DEV > BROWSER > PROCESS
```

ROS 2 的强证据——已安装节点路径、`--ros-args`、当前 `ros2 launch/run`、父链 `ros2 launch/run`——会被保护。仅有 `ROS_VERSION=2`、`AMENT_PREFIX_PATH` 这样的继承环境变量属于弱证据，因此从已经 source ROS 的终端启动 Firefox 或 VS Code，不会被自动误标成 ROS2。

systemd 方面，`user@<uid>.service` 只是用户 service manager，不代表它下面所有桌面程序都是 SYSTEMD；`todeskd.service`、`pulseaudio.service`、`org.gnome.Shell@x11.service` 这类具体 unit 仍可以正确识别为 SYSTEMD。

完整规则见 [`docs/classification.md`](docs/classification.md)。

## 环境要求

源码构建：

- Linux；首要目标环境为 Ubuntu 22.04 / x86_64
- Rust **1.88 或更高版本**
- GPU 监控可选；需要 NVIDIA 驱动暴露 NVML

使用 `.deb` 的普通用户不需要 Rust/Cargo。

## 源码构建

```bash
cargo build --release
```

不启用 NVIDIA provider 的 CPU-only 构建：

```bash
cargo build --release --no-default-features
```

生成文件：

```text
target/release/proc-lens
```

## 使用

```bash
proc-lens
proc-lens snapshot
proc-lens inspect 18452
proc-lens --type ros2
proc-lens --type dev snapshot
proc-lens --type browser snapshot
```

TYPE 支持 `ros2`、`docker`/`container`、`systemd`、`dev`、`browser`、`process`。

## 快捷键

| 按键 | 功能 |
| --- | --- |
| `j` / `↓` | 选择下一个进程 |
| `k` / `↑` | 选择上一个进程 |
| `PageDown` / `PageUp` | 翻一页 |
| `Home` / `End` | 跳到首个 / 最后一个可见进程 |
| `Enter` | 查看当前 PID 详情 |
| `/` | 搜索 command/type/project |
| `Space` | 暂停 / 恢复 |
| `r` | 暂停状态下单次刷新 |
| `t` | 切换进程树排序 |
| `c` / `m` / `g` / `p` | 按 CPU / RAM / GPU / PID 排序 |
| `?` | 显示 / 关闭帮助 |
| `q` / `Esc` | 返回 / 关闭 / 退出 |

## CPU / GPU 数值说明

顶部 CPU Gauge 是**整机 CPU 利用率**；进程 CPU 是进程级指标，在多核机器上不能直接与顶部数值相加比较，因此单个进程 CPU 数值高于当前整机百分比并不天然矛盾。

NVML 可用时，GPU 区域显示全局利用率、显存、温度和功率等信息；进程 VRAM 来自活动 graphics/compute context。驱动不一定在每个刷新周期都为每个 PID 提供新鲜的进程 GPU utilization，因此缺失值坚持显示 `-`。

## 验证

运行完整本地质量门：

```bash
./scripts/verify.sh
```

该脚本检查格式、Clippy、全 feature 测试、CPU-only 测试和 release build。GitHub Actions 还会额外使用 Rust 1.88 验证最低支持版本，并实际构建/校验 Ubuntu `.deb`。

当前性能目标仍是典型工作站上 **idle CPU <1%**、**RSS <50 MiB**。这是验收目标，不是已经发布的 benchmark 结论；正式引用前应在目标机器上实测。

## v0.2.3 范围

v0.2.3 是**纯打包版本**：增加 cargo-deb metadata、Ubuntu 桌面启动器、SVG/PNG 图标、确定性的 `.deb` 校验，以及 tag 触发的 GitHub Release 发布。它不加入 kill/renice、鼠标交互、进程族折叠/聚合、历史曲线、持久配置、Web/远程监控、Prometheus、Docker 生命周期管理、ROS graph/topic 可视化、VS Code 内部 workspace 数据库解析、日志分析、LLM 分类、AppImage/Snap/Flatpak、ARM64 release artifact、GUI wrapper 或新的运行时分类规则。

---

## License / 许可证

MIT
