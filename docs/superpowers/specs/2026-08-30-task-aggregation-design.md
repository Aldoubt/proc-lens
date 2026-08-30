# proc-lens v0.3.0 Task Aggregation Design

## Goal

Add a deterministic, explainable task-level aggregation layer above the existing per-process model so proc-lens can answer both:

- which individual PID consumes resources; and
- how much CPU, RAM, storage I/O and VRAM an application/task as a whole consumes.

The existing process view and `inspect <PID>` behavior remain available and backward-compatible.

## Scope

v0.3.0 adds:

- deterministic task ownership and grouping;
- task-level CPU, RSS, disk I/O and VRAM aggregation;
- a Task View in the TUI;
- task detail showing member processes;
- `proc-lens tasks` and `proc-lens task <TASK_ID>` CLI commands;
- tests for grouping, aggregation, selection/sorting and packaging;
- version bump and release packaging for `v0.3.0`.

v0.3.0 does not add:

- user YAML task rules;
- daemon/background monitoring;
- historical persistence or time-series databases;
- cleanup/kill actions;
- GUI frameworks;
- shared-memory de-duplication beyond clearly labeling RAM as summed RSS.

## Existing architecture to preserve

Current collection remains Linux-native and `/proc` based. `ProcessSnapshot` is still the source of PID, PPID, CPU, RSS and optional GPU data. `EnrichedProcess` continues to hold classification, project identity, parent-chain and tree metadata. Provenance remains the mechanism for inherited DEV/BROWSER ownership.

Task aggregation is a new layer derived from `AppSnapshot`; it does not replace direct classification or mutate raw per-PID evidence.

## Task model

Create a focused `src/task/` module with:

- `model.rs`: task identity, kind and aggregate structures;
- `resolver.rs`: deterministic process-to-task ownership;
- `mod.rs`: public exports and aggregation entry point.

### TaskKind

The public task kind is one of:

- `Ros2Launch`
- `SystemdService`
- `Container`
- `Application`
- `Process`

`Application` covers inherited application/project ownership such as Firefox or a development application family. `Process` is the conservative fallback.

### TaskId

`TaskId` is a stable printable string for the lifetime of the current task anchor and must not depend on row order.

Canonical forms:

- ROS 2 launcher: `ros2:<pid>:<start_time_ticks>`
- application owner: `app:<pid>:<start_time_ticks>`
- standalone process: `proc:<pid>:<start_time_ticks>`
- concrete systemd service: `systemd:<unit>`
- container: `container:<normalized-cgroup-key>`

PID-based IDs always include `start_time_ticks` so PID reuse cannot inherit an older task identity.

For containers, `normalized-cgroup-key` is the lexicographically smallest live cgroup path that matches the existing container-cgroup rule, after trimming redundant leading/trailing `/` characters. The full matching path is retained rather than guessing a shortened container ID. This is deterministic and keeps distinct container scopes separate.

### TaskSnapshot

A task snapshot contains:

- `id: TaskId`
- `kind: TaskKind`
- `label: String`
- `owner_pid: Option<i32>`
- `member_ids: Vec<ProcessIdentity>`
- `process_count: usize`
- `cpu_percent: f32`
- `rss_bytes: u64`
- `disk_read_bytes: Option<u64>`
- `disk_write_bytes: Option<u64>`
- `read_bytes_per_second: Option<u64>`
- `write_bytes_per_second: Option<u64>`
- `vram_bytes: Option<u64>`

Member order is deterministic: owner first when present, then ascending PID.

## Task ownership precedence

For every live process, resolve exactly one task using the following precedence.

### 1. ROS 2 launch/run ancestor

If the process itself or one of its live ancestors has a command containing `ros2 launch` or `ros2 run`, the nearest such launcher in the parent chain is the task anchor.

The launcher and all descendants resolved to that same anchor form one task. Two independent launchers must never be merged merely because they share a workspace or project label.

The task label should be derived conservatively from the launcher command:

- `ros2 launch <package> <launch-file>` -> `<package>/<launch-file>`
- `ros2 run <package> <executable>` -> `<package>/<executable>`
- malformed/partial command -> compact launcher command label

This matching must reuse the same semantic rule as direct ROS 2 launcher classification so classification and grouping cannot silently diverge.

### 2. Concrete systemd service

If no ROS 2 launcher owns the process and the process cgroup proves a concrete `.service` unit, group by that exact service unit.

`user@<uid>.service` remains excluded because it is only a generic per-user manager and is not a meaningful application task.

### 3. Container cgroup

If no higher-priority anchor applies and the process cgroup proves a container, group by the deterministic normalized cgroup key defined above.

Different containers must not merge even when their process names or project labels match.

### 4. DEV/BROWSER provenance owner

If no higher-priority anchor applies and provenance identifies a live owner PID for a generic child, group the child with that owner using the owner's `ProcessIdentity`.

This covers browser child processes and development application children without pretending inherited evidence belongs directly to the child PID.

The direct owner process itself uses the same `app:<pid>:<start_time_ticks>` task when it is a Browser or Development process, so owner and inherited children converge on one application task.

### 5. Standalone process fallback

If no task anchor can be proven, the process is a one-member task using its own `ProcessIdentity`.

No grouping is performed solely because two processes have the same executable name, project label, workspace, package or command text.

## Aggregation semantics

### CPU

Task CPU is the arithmetic sum of member per-process CPU percentages from the same `AppSnapshot`.

It may exceed 100% on multi-core systems. The UI must not clamp it.

For TUI presentation, task CPU uses the same EMA principle as process CPU, keyed by `TaskId`, and task reorder remains delayed to avoid row thrashing.

### RAM

Task RAM is the saturating sum of member RSS bytes.

The UI and CLI label this metric explicitly as `RSS Σ` / `RAM RSS Σ`. It is not described as unique physical memory because shared pages may be counted in more than one process RSS.

### Disk I/O

Cumulative read/write bytes and sampled rates are summed only when every current task member has a readable corresponding process I/O sample.

If any member is inaccessible or unknown, the task-level value for that metric is `None` and is displayed as `-`. Missing data is never fabricated as zero.

### VRAM

When a GPU sample is available, task VRAM is the saturating sum of every member's known per-process NVML VRAM value. Members with no GPU process entry contribute no VRAM. If no member has a per-process VRAM value, the task value is `None` and displays as `-` rather than an invented `0`.

When no GPU sample is available at all, every task VRAM value is `None`.

v0.3.0 does not sum per-process GPU utilization percentages because that is not a robust aggregate metric.

## TUI behavior

The application starts in the existing Process View.

Add `a` to toggle:

- `PROCESS` view
- `TASK` view

The top system gauges remain unchanged.

### Task table

Default columns:

`TASK | KIND | PROC | CPU% | RSS Σ | READ/s | WRITE/s | VRAM`

The label must be truncated only for display; the underlying `TaskId` and label remain intact.

Existing sort keys are reused:

- `c`: CPU
- `m`: RSS
- `g`: VRAM
- `/`: search
- `Space`: pause/resume
- `r`: manual refresh while paused
- `Enter`: detail

`p` in Task View sorts by task label/ID deterministically rather than PID.

Tree mode is process-specific. Pressing `t` in Task View has no effect and does not mutate task grouping.

### Selection stability

Process View remains anchored by PID as today.

Task View selection is anchored by `TaskId`. When a task disappears, selection falls back to the nearest surviving visible row using the same index-preservation behavior used by the process model.

Switching between Process and Task View preserves each view's independent last selection when possible.

### Search/filter

Task search matches:

- task label;
- task ID;
- task kind;
- member process name/command;
- member provenance project label.

The existing `--type` process-type filter remains a Process View filter. Task View ignores that filter in v0.3.0; v0.3.0 does not reinterpret it as a task-kind filter.

## Task detail

Task detail displays:

- task ID;
- kind;
- label;
- owner PID when applicable;
- process count;
- CPU;
- `RAM RSS Σ`;
- cumulative disk read/write;
- read/write rate;
- VRAM;
- member table containing PID, TYPE, PROJECT, CPU, RAM and command.

Unknown aggregate fields display `-`.

Task detail does not replace per-process `inspect`; users can return to Process View for full direct classification evidence and parent-chain inspection.

## CLI

Preserve:

- `proc-lens snapshot`
- `proc-lens inspect <PID>`

Add:

- `proc-lens tasks`: non-interactive task snapshot sorted by CPU, then RSS, then TaskId;
- `proc-lens task <TASK_ID>`: detail for one current task.

`task <TASK_ID>` samples using the same two-sample startup path as existing snapshot/inspect. If the task is not present, return a not-found error rather than silently selecting a similarly named task.

## Internal data flow

Per refresh:

1. `ProcCollector` samples the system and per-process I/O.
2. `enrich()` produces `EnrichedProcess` values exactly as today.
3. GPU data merges into member processes exactly as today.
4. provenance resolves for all processes.
5. task resolver assigns each process to one deterministic `TaskId`.
6. aggregator builds `Vec<TaskSnapshot>` from the same immutable `AppSnapshot` generation.
7. TUI presentation models derive smoothed/reordered rows independently for process and task views.

No second `/proc` scan is introduced by task aggregation.

## Error and race handling

Processes may exit between `/proc` reads. Existing collector behavior remains authoritative.

Task grouping only uses members present in the completed `AppSnapshot`. Missing ancestors do not cause guessing from stale state. If an anchor is no longer present, the next snapshot may resolve surviving children conservatively to a different live anchor or standalone tasks.

Task history such as EMA is keyed by `TaskId`; PID/start-time based IDs prevent PID-reuse contamination.

All byte sums use saturating integer addition.

## Files expected to change

Create:

- `src/task/mod.rs`
- `src/task/model.rs`
- `src/task/resolver.rs`
- `tests/task_tests.rs`

Modify narrowly:

- `src/lib.rs`
- `src/classifier/mod.rs` to expose/reuse ROS 2 launcher semantics
- `src/classifier/rules.rs` to expose/reuse container-cgroup semantics
- `src/app.rs`
- `src/ui/model.rs`
- `src/ui/dashboard.rs`
- `src/ui/detail.rs`
- `src/ui/mod.rs`
- `src/main.rs`
- `tests/app_tests.rs`
- `tests/ui_model_tests.rs`
- `tests/ui_state_tests.rs`
- `Cargo.toml`
- `README.md`

Packaging scripts/workflow are changed only if tests reveal a version-hardcoded assumption; the existing dynamic Cargo-version release path should otherwise be preserved.

## Test requirements

Tests must prove at minimum:

1. one ROS 2 launcher and its descendants become one task;
2. two independent ROS 2 launchers are not merged;
3. direct `ros2 run` is a valid task anchor;
4. concrete systemd service members group by exact unit;
5. generic `user@<uid>.service` does not become a task anchor;
6. different containers do not merge;
7. Firefox/DEV generic children inherit a proven live provenance owner;
8. direct Browser/DEV owner and inherited children share one application task;
9. same project label alone never merges unrelated processes;
10. PID reuse changes PID-based TaskId because `start_time_ticks` changes;
11. CPU and RSS sums are correct;
12. CPU aggregate may exceed 100%;
13. one missing member I/O sample makes the corresponding task I/O aggregate unknown;
14. VRAM aggregation never fabricates zero when unavailable;
15. task rows sort, search and preserve selection across refreshes;
16. Process View behavior and existing tests remain unchanged;
17. `tasks` and `task <TASK_ID>` CLI formatting is deterministic;
18. `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D warnings`, `cargo test --all-features`, CPU-only tests/build, and Debian packaging validation pass.

## Release acceptance

The feature is release-ready only when:

- package version is `0.3.0`;
- all tests and static checks pass;
- `./scripts/build-deb.sh` produces and validates `target/debian/proc-lens_0.3.0-1_amd64.deb`;
- README documents Process View vs Task View, aggregation semantics and CLI commands;
- no existing PID-level inspection capability is removed;
- tag `v0.3.0` is created only after the release commit is verified.

The existing tag-triggered release workflow remains the publication mechanism.