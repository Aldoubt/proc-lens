# proc-lens

[English](#english) · [中文](#中文)

`proc-lens` is a lightweight Linux process inspector for understanding **what a process is, who launched it, which application/project it belongs to, and what resources it is consuming**.

`proc-lens` 是一个轻量级 Linux 进程检查工具，重点回答：**这个进程是什么、由谁启动、属于哪个应用/项目，以及正在消耗多少资源**。

<p align="center"><img src="docs/assets/proc-lens-v0.2.2-tui.png" alt="proc-lens TUI" width="900"></p>

# English

## Ubuntu package installation

Starting with v0.2.3, Ubuntu x86_64 users can install proc-lens from the `.deb` attached to the matching GitHub Release. Rust and Cargo are not required at runtime.

```bash
sudo apt install ./proc-lens_0.2.3-1_amd64.deb
proc-lens
sudo apt remove proc-lens
```

The application menu entry opens the existing TUI in a terminal using `Terminal=true`.

Developers can build the package locally with:

```bash
cargo install cargo-deb --locked
./scripts/build-deb.sh
```

## Verification

```bash
./scripts/verify.sh
```

GitHub Actions also checks Rust 1.88 and builds/validates the Ubuntu `.deb`.

---

# 中文

## Ubuntu 软件包安装

从 v0.2.3 开始，Ubuntu x86_64 用户可以直接安装对应 GitHub Release 中的 `.deb`，运行时不需要 Rust 或 Cargo。

```bash
sudo apt install ./proc-lens_0.2.3-1_amd64.deb
proc-lens
sudo apt remove proc-lens
```

Ubuntu 应用菜单中的 proc-lens 启动器使用 `Terminal=true`，点击后会自动打开终端并进入现有 TUI。

开发者本地构建：

```bash
cargo install cargo-deb --locked
./scripts/build-deb.sh
```

## 验证

```bash
./scripts/verify.sh
```

GitHub Actions 还会检查 Rust 1.88，并构建/验证 Ubuntu `.deb`。

## License / 许可证

MIT
