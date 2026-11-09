"""Summarize a soak run: report.py OUT_DIR. Standard library only."""

import collections
import csv
import datetime
import glob
import json
import os
import re
import statistics
import sys
import time

out = sys.argv[1]
fmt = lambda ts: time.strftime("%H:%M", time.localtime(ts))
path = lambda name: os.path.join(out, name)

events = []
for line in open(path("events.log")):
    ts, what = line.split(" ", 1)
    events.append((int(ts), what.strip()))
start = events[0][0]
end = max(e[0] for e in events)
kinds = collections.Counter(w.split(" ")[0] for _, w in events)
print(f"Run: {time.ctime(start)} → {time.ctime(end)} ({(end - start) / 3600:.1f} h)")
print("  ", events[0][1])
print("Events:", ", ".join(f"{k} {v}" for k, v in sorted(kinds.items())))
for _, w in events:
    if w.startswith("upgrade"):
        print("  ", w)

# Disruptions as (begin, end, kind). An upgrade runs from SIGQUIT (quit=) until the old
# process is gone, when its event is written; runs from before quit= existed get the 30 s
# the harness waits at most. A flap runs from "down" to the next "up".
disruptions = []
for t, w in events:
    if w.startswith("upgrade "):
        m = re.search(r"\bquit=(\d+)", w)
        disruptions.append((int(m.group(1)) if m else t - 30, t, "upgrade"))
    elif w.startswith("flap") and w.endswith("down"):
        up = next((u for u, x in events if u >= t and x.startswith("flap") and x.endswith("up")), t + 30)
        disruptions.append((t, up, "flap"))


def causes(t0, t1, before=60, after=0):
    """Kinds of disruption overlapping [t0 - before, t1 + after]."""
    return sorted({k for a, b, k in disruptions if a <= t1 + after and b >= t0 - before})


# Process samples, bucketed by hour. A sample of a dead process is "ts,pid,dead".
all_rows = list(csv.DictReader(open(path("samples.csv"))))
rows = [r for r in all_rows if (r.get("rss_kb") or "").isdigit()]
dead = sum(1 for r in all_rows if r.get("rss_kb") == "dead")
print(f"\nSamples: {len(rows)} (dead process seen in {dead})")
if not rows:
    sys.exit("No usable process samples; the soak did not reach the load phase.")
cols = ["rss_kb", "fds", "threads", "upstream_conns", "ws_conns", "client_conns", "time_wait"]
hours = collections.defaultdict(list)
for r in rows:
    hours[(int(r["ts"]) - start) // 3600].append(r)
print("hour " + " ".join(f"{c:>14}" for c in cols) + "  cpu%")
for h in sorted(hours):
    rs = hours[h]
    med = [statistics.median(float(r[c]) for r in rs) for c in cols]
    # CPU: ticks (1/100 s) over the hour, per process (upgrades restart the counter).
    cpu = 0.0
    by_pid = collections.defaultdict(list)
    for r in rs:
        by_pid[r["pid"]].append(r)
    span = 0
    for pr in by_pid.values():
        if len(pr) > 1:
            cpu += float(pr[-1]["cpu_ticks"]) - float(pr[0]["cpu_ticks"])
            span += int(pr[-1]["ts"]) - int(pr[0]["ts"])
    cpu_pct = cpu / span if span else 0
    partial = "  (partial)" if len(rs) < 60 else ""
    print(f"{h:>4} " + " ".join(f"{m:>14.0f}" for m in med) + f"  {cpu_pct:5.1f}{partial}")
# Compare whole hours only: the last bucket can hold just the sample taken after the load
# stopped.
full = [h for h in sorted(hours) if len(hours[h]) >= 60]
if len(full) >= 3:
    first, last = full[1], full[-1]
    for c in ("rss_kb", "fds", "threads"):
        a = statistics.median(float(r[c]) for r in hours[first])
        b = statistics.median(float(r[c]) for r in hours[last])
        print(f"{c}: hour {first} median {a:.0f}, hour {last} median {b:.0f} ({b - a:+.0f})")


def slope(points):
    """Least-squares slope of (x, y) points, or 0 for fewer than two distinct x."""
    if len(points) < 2:
        return 0.0
    mx = statistics.fmean(x for x, _ in points)
    my = statistics.fmean(y for _, y in points)
    sxx = sum((x - mx) ** 2 for x, _ in points)
    return sum((x - mx) * (y - my) for x, y in points) / sxx if sxx else 0.0


# Upgrades start a new process, which hides growth within one process from the hourly
# medians: RSS of each process 15 minutes in, at its end, its peak, and its trend from
# 15 minutes in to the end.
print("Per process:")
procs = collections.OrderedDict()
for r in rows:
    # The run's last sample is taken after the load stopped.
    if r.get("client_conns") not in (None, "", "0"):
        procs.setdefault(r["pid"], []).append(r)
mb = lambda r: int(r["rss_kb"]) / 1024
for pid, pr in procs.items():
    t0 = int(pr[0]["ts"])
    warm = [r for r in pr if int(r["ts"]) - t0 >= 900]
    baseline = warm or pr
    baseline_min = (int(baseline[0]["ts"]) - t0) / 60
    trend = (
        f"{slope([((int(r['ts']) - t0) / 3600, mb(r)) for r in warm]):+.2f} MB/h"
        if len(warm) >= 2 else "unavailable (<2 samples after 15 min)"
    )
    age = int(pr[-1]["ts"]) - t0
    duration = f"{age / 3600:.1f} h" if age >= 3600 else f"{age / 60:.1f} min"
    print(
        f"  pid {pid}: {duration}, RSS {mb(baseline[0]):.1f} MB at {baseline_min:.0f} min, "
        f"{mb(pr[-1]):.1f} MB at the end, peak {max(mb(r) for r in pr):.1f} MB, "
        f"trend {trend}; fds {min(int(r['fds']) for r in baseline)}–{max(int(r['fds']) for r in baseline)}"
    )
print(f"max RSS {max(mb(r) for r in rows):.1f} MB, max fds {max(int(r['fds']) for r in rows)}")
print(f"access log lines dropped: {rows[-1]['log_dropped']}, idle upstream reuse lost: {rows[-1]['reuse_lost']}")

# Load generator windows.
print("\nLoad (oha):")
streams = collections.defaultdict(list)
for f in sorted(glob.glob(path("oha/*.json"))):
    name, ts = os.path.basename(f)[:-5].rsplit("-", 1)
    try:
        streams[name].append((int(ts), json.load(open(f))))
    except json.JSONDecodeError:
        print(f"  unreadable {f}")
total_req = 0
by_cause = collections.defaultdict(collections.Counter)
unexplained_windows = []
aborted = collections.Counter()
for name, ws in sorted(streams.items()):
    codes, errs = collections.Counter(), collections.Counter()
    p99 = []
    for ts, j in ws:
        c = collections.Counter({k: v for k, v in j.get("statusCodeDistribution", {}).items()})
        e = collections.Counter(j.get("errorDistribution", {}))
        # oha cancels what is still in flight when its window ends (HTTP/2 ignores -w).
        aborted[name] += e.pop("aborted due to deadline", 0)
        codes += c
        errs += e
        lat = j.get("latencyPercentiles") or {}
        if lat.get("p99") is not None:
            p99.append(lat["p99"] * 1000)
        bad = collections.Counter({k: v for k, v in c.items() if not k.startswith("2")}) + e
        if bad:
            cause = "+".join(causes(ts, ts + (j["summary"].get("total") or 300))) or "NO EVENT"
            by_cause[(name, cause)] += bad
            if cause == "NO EVENT":
                unexplained_windows.append((ts, name, dict(bad)))
    n = sum(codes.values()) + sum(errs.values())
    total_req += n
    ok = sum(v for k, v in codes.items() if k.startswith("2"))
    p = f"p99 median {statistics.median(p99):.2f} ms, worst {max(p99):.2f} ms" if p99 else ""
    print(f"  {name:6} {n:>11,} requests, {n - ok:>6,} failed ({100 * (n - ok) / max(n, 1):.4f}%), {p}")
    if aborted[name]:
        print(f"         plus {aborted[name]:,} cancelled by oha at the end of its windows")
print(f"  total {total_req:,} requests")
if by_cause:
    print("\nFailed requests by the disruption in their window:")
    for (name, cause), bad in sorted(by_cause.items()):
        print(f"  {name:6} {cause:14} {sum(bad.values()):>6}  {dict(bad)}")
    print(f"Windows with failures and no disruption: {len(unexplained_windows)}")
    for ts, name, bad in unexplained_windows[:20]:
        print(f"  {fmt(ts)} {name}: {bad}")

# WebSockets.
if os.path.exists(path("ws.csv")):
    w = list(csv.DictReader(open(path("ws.csv"))))
    if w:
        last = w[-1]
        print(
            f"\nWebSocket: {int(last['msgs']):,} echoes on long-lived connections, "
            f"{last['errors']} errors ({last['reconnects']} reconnects); short-lived "
            f"{int(last['short_ok']):,} ok, {last['short_err']} failed"
        )
    unexplained = []
    for line in open(path("ws-events.log")) if os.path.exists(path("ws-events.log")) else []:
        t = float(line.split(" ", 1)[0])
        if not any(k == "upgrade" and a - 1 <= t <= b + 30 for a, b, k in disruptions):
            unexplained.append(line.strip())
    print(f"  WebSocket errors outside an upgrade (SIGQUIT until 30 s after the old process left): {len(unexplained)}")
    for line in unexplained[:15]:
        print("   ", line)

# Access log: totals, and the lines kept because of an error or a 4xx/5xx status.
if os.path.exists(path("logcount.txt")):
    lines = sum(int(l.split()[1]) for l in open(path("logcount.txt")))
    print(f"\nAccess log: {lines:,} lines over all rotations")
if os.path.exists(path("access-errors.log")):
    kept = collections.Counter()
    for line in open(path("access-errors.log"), errors="replace"):
        try:
            d = json.loads(line)
            t = datetime.datetime.strptime(d["ts"], "%Y-%m-%dT%H:%M:%S.%fZ")
            t = t.replace(tzinfo=datetime.timezone.utc).timestamp()
        except (ValueError, KeyError):
            kept[("unreadable", "", "", "")] += 1
            continue
        cause = "+".join(causes(t, t, before=5, after=15)) or "NO EVENT"
        kept[(cause, d.get("site", ""), str(d.get("status", "")), d.get("error", ""))] += 1
    print(f"  {sum(kept.values()):,} with an error or a 4xx/5xx status (cause, site, status, error):")
    for (cause, site, status, err), n in sorted(kept.items(), key=lambda kv: -kv[1])[:20]:
        print(f"  {n:>7} {cause:14} {site:6} {status:4} {err}")

# conduit's own log.
levels = collections.Counter()
panics = []
msgs = collections.Counter()
for line in open(path("conduit-stderr.log"), errors="replace"):
    parts = line.split(None, 3)
    if len(parts) > 1 and parts[1] in ("ERROR", "WARN", "INFO", "DEBUG"):
        levels[parts[1]] += 1
        if len(parts) == 4 and parts[1] in ("ERROR", "WARN"):
            msgs[" ".join(parts[1:4])[:110]] += 1
    if "panicked" in line:
        panics.append(line.strip())
print(f"\nconduit log: {dict(levels)}; panics: {len(panics)}")
for p in panics[:5]:
    print("  ", p)
for m, n in msgs.most_common(12):
    print(f"  {n:>7} {m}")
