# proc-lens v0.3.0 Task Aggregation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add deterministic task-level process aggregation, expose it in CLI and TUI, and release it as proc-lens v0.3.0 without regressing existing PID-level inspection.

**Architecture:** Keep `/proc` collection, per-PID enrichment, classification, provenance, GPU merging, and process presentation unchanged as the source of truth. Add a new pure `task` layer that derives task ownership and aggregates resources from one immutable `AppSnapshot`; CLI and TUI consume this derived layer without introducing a second system scan.

**Tech Stack:** Rust 2024, clap 4, ratatui 0.30, crossterm 0.29, Linux `/proc`, optional NVML, cargo-deb.

**Spec:** `docs/superpowers/specs/2026-08-30-task-aggregation-design.md`

## Global Constraints

- Target release version is exactly `0.3.0`.
- Preserve `proc-lens snapshot`, `proc-lens inspect <PID>`, default Process View, and existing `--type` semantics.
- Task grouping is deterministic and never groups solely by executable name, project label, workspace, package, or command text.
- PID-backed task IDs include `(pid,start_time_ticks)` to avoid PID-reuse contamination.
- RAM is explicitly summed RSS and must be labelled `RSS Σ` / `RAM RSS Σ`.
- Missing task I/O data is unknown (`None` / `-`), never fabricated as zero.
- Missing per-process VRAM must never cause an invented task-level `0`.
- Task aggregation performs no second `/proc` scan.
- Existing tag-triggered dynamic Debian release workflow remains the publication mechanism unless verification proves a hard-coded assumption.

---

### Task 1: Task domain model, ownership resolver, and aggregate semantics

**Files:**
- Create: `src/task/model.rs`
- Create: `src/task/resolver.rs`
- Create: `src/task/mod.rs`
- Create: `tests/task_tests.rs`
- Modify: `src/lib.rs`
- Modify: `src/classifier/mod.rs`
- Modify: `src/classifier/rules.rs`

**Interfaces:**
- Consumes: `app::AppSnapshot`, `app::EnrichedProcess`, `provenance::resolve_all_provenance`, `collector::process::ProcessIoSnapshot`, `process::ProcessIdentity`.
- Produces: `TaskId`, `TaskKind`, `TaskSnapshot`, `aggregate_tasks(&AppSnapshot) -> Vec<TaskSnapshot>`, `members_for_task<'a>(&TaskSnapshot, &'a AppSnapshot) -> Vec<&'a EnrichedProcess>`.
- Reuses: public `classifier::is_ros2_launcher(&ProcessSnapshot) -> bool` and public `classifier::is_container_cgroup(&str) -> bool` so grouping and direct classification share the same predicates.

- [ ] **Step 1: Write failing task resolver tests**

Add tests that construct `AppSnapshot` fixtures and assert:

```rust
#[test]
fn ros2_launcher_and_descendants_share_one_task() {
    let snapshot = fixture_ros2_launch_tree();
    let tasks = aggregate_tasks(&snapshot);
    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0].kind, TaskKind::Ros2Launch);
    assert_eq!(tasks[0].process_count, 3);
    assert_eq!(tasks[0].label, "agt_navigation/bringup.launch.py");
}

#[test]
fn independent_ros2_launchers_do_not_merge() {
    let snapshot = fixture_two_launchers_same_workspace();
    let tasks = aggregate_tasks(&snapshot);
    assert_eq!(tasks.len(), 2);
    assert_ne!(tasks[0].id, tasks[1].id);
}

#[test]
fn pid_reuse_changes_pid_backed_task_id() {
    let first = aggregate_tasks(&fixture_single_process(77, 100));
    let second = aggregate_tasks(&fixture_single_process(77, 200));
    assert_ne!(first[0].id, second[0].id);
}
```

Also cover direct `ros2 run`, concrete systemd units, exclusion of `user@<uid>.service`, distinct container cgroups, Browser/DEV owner plus inherited children, and the rule that same project label alone never merges unrelated processes.

- [ ] **Step 2: Run the resolver tests and verify RED**

Run:

```bash
cargo test --test task_tests --no-default-features
```

Expected: compilation fails because `proc_lens::task`, `TaskKind`, and `aggregate_tasks` do not exist yet.

- [ ] **Step 3: Implement minimal task model and ownership resolution**

Implement:

```rust
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TaskId(pub String);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TaskKind {
    Ros2Launch,
    SystemdService,
    Container,
    Application,
    Process,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TaskSnapshot {
    pub id: TaskId,
    pub kind: TaskKind,
    pub label: String,
    pub owner_pid: Option<i32>,
    pub member_ids: Vec<ProcessIdentity>,
    pub process_count: usize,
    pub cpu_percent: f32,
    pub rss_bytes: u64,
    pub disk_read_bytes: Option<u64>,
    pub disk_write_bytes: Option<u64>,
    pub read_bytes_per_second: Option<u64>,
    pub write_bytes_per_second: Option<u64>,
    pub vram_bytes: Option<u64>,
}
```

Ownership resolution order must be: nearest live ROS2 launcher → concrete systemd unit → normalized container cgroup → Browser/DEV provenance owner → standalone process. Direct Browser/DEV owners must resolve to their own `app:<pid>:<start_time_ticks>` task so generic inherited children converge on the same ID.

- [ ] **Step 4: Run resolver tests and verify GREEN**

Run:

```bash
cargo test --test task_tests --no-default-features
cargo test --test classifier_tests --test provenance_tests --no-default-features
```

Expected: all listed tests pass.

- [ ] **Step 5: Add failing aggregate metric tests**

Add explicit tests such as:

```rust
#[test]
fn task_cpu_can_exceed_one_hundred_and_rss_is_summed() {
    let snapshot = fixture_task_with_resources([80.0, 65.0], [512, 1024]);
    let task = aggregate_tasks(&snapshot).remove(0);
    assert_eq!(task.cpu_percent, 145.0);
    assert_eq!(task.rss_bytes, 1536);
}

#[test]
fn missing_member_io_makes_task_io_unknown() {
    let snapshot = fixture_task_with_one_missing_io_member();
    let task = aggregate_tasks(&snapshot).remove(0);
    assert_eq!(task.read_bytes_per_second, None);
    assert_eq!(task.write_bytes_per_second, None);
}

#[test]
fn no_member_vram_does_not_become_zero() {
    let snapshot = fixture_task_without_vram();
    let task = aggregate_tasks(&snapshot).remove(0);
    assert_eq!(task.vram_bytes, None);
}
```

- [ ] **Step 6: Run metric tests and verify RED**

Run:

```bash
cargo test --test task_tests --no-default-features
```

Expected: new metric assertions fail because aggregate fields are not implemented yet.

- [ ] **Step 7: Implement aggregate metrics with saturating byte sums**

CPU uses arithmetic `f32` sum. RSS uses `saturating_add`. Each I/O metric returns `Some(sum)` only when every member has that specific readable value. VRAM returns `Some(sum)` only when at least one member has a known per-process VRAM value; otherwise `None`.

- [ ] **Step 8: Verify Task 1**

Run:

```bash
cargo fmt --check
cargo test --test task_tests --no-default-features
cargo test --test classifier_tests --test provenance_tests --no-default-features
```

Expected: all pass with no warnings/errors.

- [ ] **Step 9: Commit Task 1**

```bash
git add src/task src/lib.rs src/classifier tests/task_tests.rs
git commit -m "feat: add deterministic task aggregation"
```

---

### Task 2: CLI task snapshot and task detail

**Files:**
- Modify: `src/app.rs`
- Modify: `src/main.rs`
- Modify: `tests/app_tests.rs`
- Modify: `tests/task_tests.rs`

**Interfaces:**
- Consumes: `task::aggregate_tasks`, `task::members_for_task`, existing `format_bytes` and provenance resolution.
- Produces: `format_tasks(&AppSnapshot) -> String`, `format_task(&AppSnapshot, &TaskId) -> Option<String>`, clap subcommands `Tasks` and `Task { task_id: String }`.

- [ ] **Step 1: Write failing formatter/CLI-contract tests**

Add tests asserting deterministic headers, sorting, summed-RSS label, exact ID lookup, and unknown values:

```rust
#[test]
fn task_snapshot_sorts_by_cpu_then_rss_then_id() {
    let output = format_tasks(&fixture_multiple_tasks());
    let lines: Vec<_> = output.lines().collect();
    assert!(lines.iter().any(|line| line.contains("RSS Σ")));
    assert!(position(&output, "task-fast") < position(&output, "task-slow"));
}

#[test]
fn task_detail_requires_exact_task_id() {
    let snapshot = fixture_multiple_tasks();
    assert!(format_task(&snapshot, &TaskId("missing".into())).is_none());
}
```

- [ ] **Step 2: Verify RED**

Run:

```bash
cargo test --test app_tests --test task_tests --no-default-features
```

Expected: compilation fails because `format_tasks` / `format_task` do not exist.

- [ ] **Step 3: Implement formatters and clap commands**

Add:

```rust
enum Command {
    Snapshot,
    Inspect { pid: i32 },
    Tasks,
    Task { task_id: String },
}
```

`tasks` uses the existing two-sample startup path. `task <TASK_ID>` constructs an exact `TaskId` and returns `NotFound` when absent.

- [ ] **Step 4: Verify GREEN and regression safety**

Run:

```bash
cargo test --test app_tests --test task_tests --no-default-features
cargo test --all-targets --no-default-features
```

Expected: all pass.

- [ ] **Step 5: Commit Task 2**

```bash
git add src/app.rs src/main.rs tests/app_tests.rs tests/task_tests.rs
git commit -m "feat: expose task aggregation in cli"
```

---

### Task 3: TUI Process/Task view toggle, task presentation, and task detail

**Files:**
- Modify: `src/app.rs`
- Modify: `src/ui/model.rs`
- Modify: `src/ui/dashboard.rs`
- Modify: `src/ui/detail.rs`
- Modify: `src/ui/mod.rs`
- Modify: `tests/ui_model_tests.rs`
- Modify: `tests/ui_state_tests.rs`

**Interfaces:**
- Consumes: `task::aggregate_tasks`, `TaskId`, `TaskSnapshot`, existing `SortMode`, existing per-process presentation model.
- Produces: `EntityMode::{Process,Task}` in UI state; task row model with EMA keyed by `TaskId`; independent process PID and task ID selection; `a` toggle behavior; Task View renderer and Task Detail renderer.

- [ ] **Step 1: Write failing UI state tests**

Add tests that prove default is Process View, `a` toggles mode, task selection is independent of PID selection, and `t` does nothing in Task View.

Example target behavior:

```rust
#[test]
fn ui_defaults_to_process_mode() {
    assert_eq!(UiState::default().entity_mode, EntityMode::Process);
}

#[test]
fn task_and_process_selection_are_independent() {
    let mut state = UiState::default();
    state.selected_pid = Some(42);
    state.selected_task_id = Some(TaskId("task-a".into()));
    assert_eq!(state.selected_pid, Some(42));
    assert_eq!(state.selected_task_id.as_deref(), Some(&TaskId("task-a".into())));
}
```

- [ ] **Step 2: Verify RED**

Run:

```bash
cargo test --test ui_state_tests --test ui_model_tests --no-default-features
```

Expected: compilation fails because task-view UI state/model does not exist.

- [ ] **Step 3: Implement task presentation model with EMA and stable ordering**

Keep existing process smoothing untouched. Add task CPU smoothing keyed by `TaskId`, task ordered IDs, prior ranks, search matching, selection reconciliation, and sorting:

- CPU: same alpha/banding principle as process rows;
- Memory: `rss_bytes` descending;
- GPU: `vram_bytes` descending, unknown last;
- `p`: task label then ID ascending.

- [ ] **Step 4: Add failing behavior tests for refresh, search, sorting and disappearing tasks**

Tests must prove task selection survives a refresh when the same `TaskId` remains, falls to nearest row when removed, search matches member command/project, and CPU order does not thrash inside the existing 2-point band.

- [ ] **Step 5: Verify RED, then implement behavior**

Run before implementation:

```bash
cargo test --test ui_model_tests --no-default-features
```

Expected: new assertions fail.

Implement only the behavior required by those assertions, then rerun until green.

- [ ] **Step 6: Implement rendering and keyboard routing**

Task table columns are exactly:

```text
TASK | KIND | PROC | CPU% | RSS Σ | READ/s | WRITE/s | VRAM
```

`a` toggles Process/Task view. `Enter` opens the selected entity detail. `t` only toggles tree mode in Process View. Existing pause, refresh, search and sort keys remain unchanged. Task View ignores `--type` filtering.

- [ ] **Step 7: Verify Task 3**

Run:

```bash
cargo fmt --check
cargo test --test ui_state_tests --test ui_model_tests --no-default-features
cargo test --all-targets --no-default-features
```

Expected: all pass.

- [ ] **Step 8: Commit Task 3**

```bash
git add src/app.rs src/ui tests/ui_model_tests.rs tests/ui_state_tests.rs
git commit -m "feat: add task view to tui"
```

---

### Task 4: Release documentation, versioning, and Debian package metadata

**Files:**
- Modify: `Cargo.toml`
- Modify: `README.md`
- Modify only if necessary after verification: `scripts/build-deb.sh`, `.github/workflows/release.yml`, `tests/packaging_test.sh`

**Interfaces:**
- Consumes: completed CLI/TUI behavior.
- Produces: package version `0.3.0`, bilingual user documentation, dynamic `.deb` path `target/debian/proc-lens_0.3.0-1_amd64.deb`.

- [ ] **Step 1: Write/update packaging expectation before version change**

Ensure packaging tests derive expected package version from `Cargo.toml` rather than embedding `0.2.x`. If a hard-coded version is found, update the test first so it fails against the intended `0.3.0` package name.

- [ ] **Step 2: Verify RED where applicable**

Run:

```bash
bash tests/packaging_test.sh
```

If the packaging test is static/source-level, confirm it fails specifically on the old version expectation before modifying production/package metadata. If it is already dynamic, record that no production workflow change is required.

- [ ] **Step 3: Bump Cargo version and document v0.3.0**

Set:

```toml
version = "0.3.0"
```

README must document:

- Process View vs Task View;
- `a` key;
- task ownership precedence;
- `RSS Σ` caveat;
- unknown I/O/VRAM semantics;
- `proc-lens tasks`;
- `proc-lens task <TASK_ID>`;
- Ubuntu `.deb` filename for v0.3.0.

- [ ] **Step 4: Verify docs/version/package source checks**

Run:

```bash
cargo metadata --no-deps --format-version 1
bash tests/packaging_test.sh
```

Expected: metadata reports `proc-lens 0.3.0`; packaging source checks pass.

- [ ] **Step 5: Commit Task 4**

```bash
git add Cargo.toml README.md scripts/build-deb.sh .github/workflows/release.yml tests/packaging_test.sh
git commit -m "release: prepare proc-lens v0.3.0"
```

---

### Task 5: Full release verification and branch handoff

**Files:**
- No intended production changes; fix only defects exposed by verification, using a new failing regression test before each fix.

**Interfaces:**
- Consumes: all previous tasks.
- Produces: verified v0.3.0 branch ready for merge/tag.

- [ ] **Step 1: Run formatting and clippy**

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
```

Expected: PASS.

- [ ] **Step 2: Run full feature and CPU-only test/build matrix**

```bash
cargo test --all-features
cargo test --all-targets --no-default-features
cargo build --release --no-default-features
```

Expected: PASS.

- [ ] **Step 3: Build and validate the Debian package**

Install the pinned packaging tool if absent:

```bash
cargo install cargo-deb --locked --version 3.7.0
./scripts/build-deb.sh
```

Expected artifact:

```text
target/debian/proc-lens_0.3.0-1_amd64.deb
```

Expected: existing `dpkg-deb` validation passes for package name, version, architecture, binary, desktop entry, icons, README and LICENSE.

- [ ] **Step 4: Smoke-check CLI behavior**

```bash
./target/release/proc-lens snapshot | head
./target/release/proc-lens tasks | head
```

For a current TaskId emitted by `tasks`, run:

```bash
./target/release/proc-lens task '<TASK_ID>'
```

Expected: deterministic task summary/detail with `RSS Σ`; unknown metrics print `-`.

- [ ] **Step 5: Compare feature branch with main and verify only intended scope changed**

```bash
git diff --stat main...HEAD
git diff --check main...HEAD
```

Expected: no whitespace errors and no unrelated refactor.

- [ ] **Step 6: Final verification commit only if needed**

If verification required a defect fix, commit it with a focused message after its regression test passes. Otherwise leave the existing task commits unchanged.

- [ ] **Step 7: Do not tag until branch completion is approved**

The branch is ready when all commands above pass. `v0.3.0` must be created only after branch completion/merge decision; pushing the tag will trigger the existing release workflow and publish `proc-lens_0.3.0-1_amd64.deb`.
