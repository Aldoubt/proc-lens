#!/usr/bin/env python3
"""Validate and summarize a proc-lens runtime snapshot."""

from __future__ import annotations

import argparse
import json
import sys
from collections import Counter
from pathlib import Path
from typing import Any


def fail(message: str) -> None:
    raise ValueError(message)


def require_object(value: Any, name: str) -> dict[str, Any]:
    if not isinstance(value, dict):
        fail(f"{name} must be an object")
    return value


def require_array(value: Any, name: str) -> list[Any]:
    if not isinstance(value, list):
        fail(f"{name} must be an array")
    return value


def number(value: Any, name: str) -> int | float:
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        fail(f"{name} must be a number")
    return value


def positive_integer(value: Any, name: str) -> int:
    number_value = number(value, name)
    if not isinstance(number_value, int) or number_value <= 0:
        fail(f"{name} must be a positive integer")
    return number_value


def optional_value_stats(objects: list[dict[str, Any]], fields: list[str]) -> dict[str, int]:
    stats: dict[str, int] = {}
    for field in fields:
        null_count = sum(item.get(field) is None for item in objects)
        unknown_count = sum(
            isinstance(item.get(field), str)
            and item.get(field, "").strip().lower() in {"", "unknown", "-"}
            for item in objects
        )
        stats[field] = null_count + unknown_count
    return stats


def cpu_sort_key(item: dict[str, Any]) -> float:
    value = item.get("cpu_percent")
    return float(value) if isinstance(value, (int, float)) else -1.0


def text_report(report: dict[str, Any]) -> str:
    lines = [
        f"status: {report['status']}",
        f"schema_version: {report['schema_version']}",
        f"logical_cpus: {report['logical_cpus']}",
        f"process count: {report['process_count']}",
        f"thread count: {report['thread_count']}",
        f"threads >= processes: {report['thread_count'] >= report['process_count']} (reported only)",
        f"ros_nodes: {report['ros_node_count']} (NOT IMPLEMENTED / EXPECTED EMPTY)",
        f"topics: {report['topic_count']} (NOT IMPLEMENTED / EXPECTED EMPTY)",
        f"edges: {report['edge_count']} (NOT IMPLEMENTED / EXPECTED EMPTY)",
        f"findings: {report['finding_count']} (NOT IMPLEMENTED / EXPECTED EMPTY)",
        "",
        "Top 20 CPU processes:",
    ]
    lines.extend(
        f"  {item['pid']:>6} {item['cpu_percent']!s:>8} {item['name']} [{item['process_type']}]"
        for item in report["top_cpu_processes"]
    )
    lines.append("Top 20 CPU threads:")
    lines.extend(
        f"  {item['pid']:>6}/{item['tid']:<6} {item['cpu_percent']!s:>8} {item['name']}"
        for item in report["top_cpu_threads"]
    )
    lines.append("Top 20 thread-count processes:")
    lines.extend(
        f"  {item['pid']:>6} {item['thread_count']:>6} {item['name']}"
        for item in report["top_thread_count_processes"]
    )
    lines.extend(["scheduler distribution:"])
    lines.extend(f"  {key}: {value}" for key, value in report["scheduler_distribution"].items())
    lines.append("CPU core distribution:")
    lines.extend(f"  {key}: {value}" for key, value in report["cpu_core_distribution"].items())
    lines.append("null/unknown field counts:")
    lines.extend(f"  {key}: {value}" for key, value in report["null_unknown_fields"].items())
    if report["warnings"]:
        lines.append("warnings:")
        lines.extend(f"  WARNING: {warning}" for warning in report["warnings"])
    return "\n".join(lines) + "\n"


def build_report(snapshot: dict[str, Any]) -> dict[str, Any]:
    if snapshot.get("schema_version") != 1:
        fail("schema_version must equal 1")
    host = require_object(snapshot.get("host"), "host")
    arch = host.get("arch")
    if not isinstance(arch, str) or not arch.strip():
        fail("host.arch must be non-empty")
    logical_cpus = positive_integer(host.get("logical_cpus"), "host.logical_cpus")
    processes_raw = require_array(snapshot.get("processes"), "processes")
    threads_raw = require_array(snapshot.get("threads"), "threads")
    processes = [require_object(item, "processes[]") for item in processes_raw]
    threads = [require_object(item, "threads[]") for item in threads_raw]
    if not processes:
        fail("processes must not be empty")
    if not threads:
        fail("threads must not be empty")

    process_pids: set[int] = set()
    for item in processes:
        pid = positive_integer(item.get("pid"), "process.pid")
        process_pids.add(pid)
        if not isinstance(item.get("name"), str):
            fail("process.name must be a string")
    warnings: list[str] = []
    missing_thread_processes: set[int] = set()
    for item in threads:
        pid = positive_integer(item.get("pid"), "thread.pid")
        positive_integer(item.get("tid"), "thread.tid")
        if pid not in process_pids:
            missing_thread_processes.add(pid)
    if missing_thread_processes:
        warnings.append(
            "thread pid(s) not present in process snapshot (kernel/process race): "
            + ", ".join(str(pid) for pid in sorted(missing_thread_processes))
        )

    ros_nodes = require_array(snapshot.get("ros_nodes"), "ros_nodes")
    topics = require_array(snapshot.get("topics"), "topics")
    edges = require_array(snapshot.get("edges"), "edges")
    findings = require_array(snapshot.get("findings"), "findings")
    for name, values in (("ros_nodes", ros_nodes), ("topics", topics), ("edges", edges), ("findings", findings)):
        if values:
            warnings.append(f"{name} is populated; inspect runtime implementation coverage")

    scheduler = Counter(str(item["scheduler"]) if item.get("scheduler") is not None else "null" for item in threads)
    cpu_core = Counter(str(item["last_cpu"]) if item.get("last_cpu") is not None else "null" for item in threads)
    report = {
        "status": "pass",
        "schema_version": snapshot["schema_version"],
        "logical_cpus": logical_cpus,
        "process_count": len(processes),
        "thread_count": len(threads),
        "ros_node_count": len(ros_nodes),
        "topic_count": len(topics),
        "edge_count": len(edges),
        "finding_count": len(findings),
        "top_cpu_processes": sorted(processes, key=cpu_sort_key, reverse=True)[:20],
        "top_cpu_threads": sorted(threads, key=cpu_sort_key, reverse=True)[:20],
        "top_thread_count_processes": sorted(processes, key=lambda item: item.get("thread_count", 0), reverse=True)[:20],
        "scheduler_distribution": dict(sorted(scheduler.items())),
        "cpu_core_distribution": dict(sorted(cpu_core.items(), key=lambda pair: pair[0])),
        "null_unknown_fields": {
            "processes": optional_value_stats(processes, ["project", "cpu_percent"]),
            "threads": optional_value_stats(
                threads,
                ["cpu_percent", "last_cpu", "priority", "scheduler", "cpu_affinity", "voluntary_context_switches", "involuntary_context_switches"],
            ),
        },
        "warnings": warnings,
    }
    return report


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("snapshot", type=Path)
    parser.add_argument("--report-json", type=Path)
    parser.add_argument("--report-text", type=Path)
    args = parser.parse_args()
    try:
        snapshot = json.loads(args.snapshot.read_text())
        report = build_report(require_object(snapshot, "snapshot"))
    except (OSError, json.JSONDecodeError, ValueError) as error:
        print(json.dumps({"status": "fail", "error": str(error)}, indent=2), file=sys.stderr)
        return 1

    report_json = json.dumps(report, indent=2) + "\n"
    report_text = text_report(report)
    if args.report_json:
        args.report_json.write_text(report_json)
    if args.report_text:
        args.report_text.write_text(report_text)
    print(report_json, end="")
    print(report_text, file=sys.stderr, end="")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())