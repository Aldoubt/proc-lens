# proc-lens

[English](#english) · [中文](#中文)

`proc-lens` is a lightweight Linux process inspector for understanding **what a process is, who launched it, which application/project it belongs to, and what resources it is consuming**.

`proc-lens` 是一个轻量级 Linux 进程检查工具，重点回答：**这个进程是什么、由谁启动、属于哪个应用/项目，以及正在消耗多少资源**。

<p align="center">
  <img src="docs/assets/proc-lens-v0.2.2-tui.png" alt="proc-lens TUI" width="900">
</p>

# English

## Ubuntu package installation

Starting with v0.2.3, Ubuntu x86_64 users can install proc-lens from the `.deb` attached to the matching GitHub Release. Rust and Cargo are not required at runtime.

```bash
sudo apt install ./proc-lens_0.2.3-1_amd64.deb
proc-lens
sudo apt remove proc-lens
```

Open the Ubuntu application menu, search for **proc-lens**, and click the icon. The desktop entry uses `Terminal=true`, so the existing TUI opens in a terminal without a GUI wrapper.

Developers can build the package locally with:

```bash
cargo install cargo-deb --locked
./scripts/build-deb.sh
```

## Core behavior

proc-lens keeps the v0.2.x runtime behavior: stable PID-anchored TUI selection, explainable direct classification, Browser/DEV display provenance, Git-aware project labels, conservative DEV family fallback, optional NVML GPU metrics, and raw `inspect` evidence.

## Verification

```bash
./scripts/verify.sh
```

GitHub Actions also verifies Rust 1.88 and the Ubuntu `.deb` packaging path.

# 中文

## Ubuntu 软件包安装

从 v0.2.3 开始，Ubuntu x86_64 用户可以直接安装对应 GitHub Release 中的 `.deb`，运行时不需要 Rust 或 Cargo。

```bash
sudo apt install ./proc-lens_0.2.3-1_amd64.deb
proc-lens
sudo apt remove proc-lens
```

在 Ubuntu 应用菜单搜索 **proc-lens** 并点击图标即可。桌面启动器使用 `Terminal=true`，会自动打开终端并进入现有 TUI，不额外套 GUI。

开发者本地构建：

```bash
cargo install cargo-deb --locked
./scripts/build-deb.sh
```

## 核运行逻辑

v0.2.3 不修改 v0.2.x 的核心逻辑：PID 锚定稳定交互、可解释直接分类、Browser/DEV 显示归属、Git 项目标记、保守 DEV 应用族回退、可选 NVML GPU 指标以及 `inspect` 原始证据均保持不变。

## 验证

```bash
./scripts/verify.sh
```

GitHub Actions 还会验证 Rust 1.88 和 Ubuntu `.deb` 打包链路。

## License / 许可证

MIT
