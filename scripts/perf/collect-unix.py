#!/usr/bin/env python3
"""Collect a process tree on macOS or Linux using only platform tools and stdlib."""
import argparse
import csv
import json
import os
import platform
import subprocess
import time
from pathlib import Path

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--pid", type=int, required=True)
parser.add_argument("--load", required=True)
parser.add_argument("--output", type=Path, required=True)
parser.add_argument("--seconds", type=int, default=40)
parser.add_argument("--build", default="release")
args = parser.parse_args()
system = platform.system().lower()
if system not in ("darwin", "linux"):
    raise SystemExit("Use collect-windows.ps1 on Windows")
known = {args.pid}
samples = []
start = time.monotonic()
while time.monotonic() - start < args.seconds:
    tick = time.monotonic()
    output = subprocess.check_output(
        ["ps", "-axo", "pid=,ppid=,pcpu=,rss=,comm="], text=True)
    processes = [line.strip().split(None, 4) for line in output.splitlines()]
    for _ in range(len(processes)):
        before = len(known)
        known.update(int(p[0]) for p in processes if int(p[1]) in known)
        if len(known) == before:
            break
    members = [p for p in processes if int(p[0]) in known]
    sample = {"elapsed_s": tick - start, "members": members,
              "cpu_percent_ps": sum(float(p[2]) for p in members),
              "rss_bytes": sum(int(p[3]) * 1024 for p in members)}
    if system == "linux":
        # /proc CPU ticks provide interval CPU, unlike ps's lifetime average.
        ticks, pss = 0, 0
        for p in members:
            try:
                stat = Path(f"/proc/{p[0]}/stat").read_text().rsplit(")", 1)[1].split()
                ticks += int(stat[11]) + int(stat[12])
                rollup = Path(f"/proc/{p[0]}/smaps_rollup").read_text()
                pss += next(int(s.split()[1]) * 1024 for s in rollup.splitlines()
                            if s.startswith("Pss:"))
            except (OSError, StopIteration):
                sample["proc_incomplete"] = True
        sample.update(cpu_ticks=ticks, pss_bytes=pss)
    else:
        # footprint permissions vary; retain output and errors rather than guessing bytes.
        footprint = subprocess.run(["footprint", "-p", ",".join(p[0] for p in members)],
                                   capture_output=True, text=True, check=False)
        sample["footprint"] = {"exit": footprint.returncode,
                               "stdout": footprint.stdout, "stderr": footprint.stderr}
    samples.append(sample)
    time.sleep(max(0, 1 - (time.monotonic() - tick)))
args.output.with_suffix(".json").write_text(json.dumps(samples, indent=2))
metrics = {"rss_mean": (sum(s["rss_bytes"] for s in samples) / len(samples), "bytes"),
           "rss_peak": (max(s["rss_bytes"] for s in samples), "bytes")}
if system == "linux" and len(samples) > 1:
    cpu = [(b["cpu_ticks"] - a["cpu_ticks"]) / os.sysconf("SC_CLK_TCK") /
           (b["elapsed_s"] - a["elapsed_s"]) * 100 / (os.cpu_count() or 1)
           for a, b in zip(samples, samples[1:])]
    # An exiting process removes its cumulative ticks; that interval is not usable.
    cpu = [value for value in cpu if value >= 0]
    if cpu:
        metrics["cpu_percent_machine_mean"] = (sum(cpu) / len(cpu), "%")
    metrics["pss_mean"] = (sum(s["pss_bytes"] for s in samples) / len(samples), "bytes")
else:
    metrics["cpu_percent_ps_mean"] = (sum(s["cpu_percent_ps"] for s in samples) /
                                       len(samples), "% one core; ps lifetime average")
with args.output.open("w", newline="") as target:
    writer = csv.writer(target)
    writer.writerow(["平台", "构建", "负载", "指标", "数值", "单位", "样本数", "备注"])
    for metric, (value, unit) in metrics.items():
        writer.writerow([system, args.build, args.load, metric, value, unit, len(samples),
                         "GPU memory unavailable; raw process membership in JSON"])
