from pathlib import Path

path = Path("src/app.rs")
text = path.read_text()

old_import = "use crate::classifier::{Classification, ProcessType, classify};"
new_import = "use crate::classifier::{Classification, ProcessType, classify, systemd_service_unit};"
old_unit = '''fn systemd_unit(process: &EnrichedProcess) -> Option<String> {
    process
        .snapshot
        .cgroup
        .iter()
        .flat_map(|path| path.split('/'))
        .find(|part| part.ends_with(".service"))
        .map(str::to_owned)
}'''
new_unit = '''fn systemd_unit(process: &EnrichedProcess) -> Option<String> {
    process
        .snapshot
        .cgroup
        .iter()
        .find_map(|path| systemd_service_unit(path).map(str::to_owned))
}'''

if old_import not in text:
    raise SystemExit("expected classifier import not found")
if old_unit not in text:
    raise SystemExit("expected systemd_unit block not found")

text = text.replace(old_import, new_import, 1)
text = text.replace(old_unit, new_unit, 1)
path.write_text(text)
