"""Summarize a soak run: report.py OUT_DIR. Standard library only."""

import collections
import csv
import glob
import json
import os
import statistics
import sys
import time

out = sys.argv[1]
fmt = lambda ts: time.strftime("%H:%M", time.localtime(ts))

events = []
for line in open(os.path.join(out, "events.log")):
    ts, what = line.split(" ", 1)
    events.append((int(ts), what.strip()))
start = events[0][0]
end = max(e[0] for e in events)
kinds = collections.Counter(w.split(" ")[0] for _, w in events)
print(f"Run: {time.ctime(start)} → {time.ctime(end)} ({(end - start) / 3600:.1f} h)")
print("Events:", ", ".join(f"{k} {v}" for k, v in sorted(kinds.items())))
for _, w in events:
    if w.startswith("upgrade"):
        print("  ", w)
disruptive = [(t, w) for t, w in events if w.startswith(("flap", "upgrade"))]


def near(t0, t1):
    """Disruptive events within a window, with 60 s of slack after each."""
    return [f"{fmt(t)} {w}" for t, w in disruptive if t0 - 60 <= t <= t1]


# Process samples, bucketed by hour.
rows = [r for r in csv.DictReader(open(os.path.join(out, "samples.csv"))) if r.get("rss_kb")]
dead = sum(1 for r in csv.DictReader(open(os.path.join(out, "samples.csv"))) if r.get("rss_kb") is None or r.get("fds") is None)
print(f"\nSamples: {len(rows)} (dead process seen in {dead})")
cols = ["rss_kb", "fds", "threads", "upstream_conns", "ws_conns", "client_conns", "time_wait"]
hours = collections.defaultdict(list)
for r in rows:
    hours[(int(r["ts"]) - start) // 3600].append(r)
print("hour " + " ".join(f"{c:>14}" for c in cols) + "  cpu%")
prev = None
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
    print(f"{h:>4} " + " ".join(f"{m:>14.0f}" for m in med) + f"  {cpu_pct:5.1f}")
if len(hours) >= 3:
    first, last = sorted(hours)[1], sorted(hours)[-1]
    for c in ("rss_kb", "fds", "threads"):
        a = statistics.median(float(r[c]) for r in hours[first])
        b = statistics.median(float(r[c]) for r in hours[last])
        print(f"{c}: hour {first} median {a:.0f}, hour {last} median {b:.0f} ({b - a:+.0f})")
print(f"max RSS {max(int(r['rss_kb']) for r in rows) / 1024:.1f} MB, max fds {max(int(r['fds']) for r in rows)}")
print(f"access log lines dropped: {rows[-1]['log_dropped']}, idle upstream reuse lost: {rows[-1]['reuse_lost']}")

# Load generator windows.
print("\nLoad (oha):")
streams = collections.defaultdict(list)
for f in sorted(glob.glob(os.path.join(out, "oha", "*.json"))):
    name, ts = os.path.basename(f)[:-5].rsplit("-", 1)
    try:
        streams[name].append((int(ts), json.load(open(f))))
    except json.JSONDecodeError:
        print(f"  unreadable {f}")
total_req = 0
bad_windows = []
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
        failed = sum(v for k, v in c.items() if not k.startswith("2")) + sum(e.values())
        if failed:
            dur = j["summary"].get("total") or 300
            bad_windows.append((ts, name, failed, dict(c), dict(e), near(ts, ts + dur)))
    n = sum(codes.values()) + sum(errs.values())
    total_req += n
    ok = sum(v for k, v in codes.items() if k.startswith("2"))
    p = f"p99 median {statistics.median(p99):.2f} ms, worst {max(p99):.2f} ms" if p99 else ""
    print(f"  {name:6} {n:>11,} requests, {n - ok:>6,} failed ({100 * (n - ok) / max(n, 1):.4f}%), {p}")
    if n - ok:
        print(f"         statuses {dict(codes)} errors {dict(errs)}")
    if aborted[name]:
        print(f"         plus {aborted[name]:,} cancelled by oha at the end of its windows")
print(f"  total {total_req:,} requests")
if bad_windows:
    print("\nWindows with failures (and disruptive events in them):")
    for ts, name, failed, c, e, ev in bad_windows:
        bad = {k: v for k, v in c.items() if not k.startswith("2")}
        print(f"  {fmt(ts)} {name}: {failed} failed {bad} {e} ← {ev or 'NO EVENT'}")

# WebSockets.
wpath = os.path.join(out, "ws.csv")
if os.path.exists(wpath):
    w = list(csv.DictReader(open(wpath)))
    if w:
        last = w[-1]
        print(
            f"\nWebSocket: {int(last['msgs']):,} echoes on long-lived connections, "
            f"{last['errors']} errors ({last['reconnects']} reconnects); short-lived "
            f"{int(last['short_ok']):,} ok, {last['short_err']} failed"
        )
    unexplained = []
    for line in open(os.path.join(out, "ws-events.log")) if os.path.exists(os.path.join(out, "ws-events.log")) else []:
        t = float(line.split(" ", 1)[0])
        if not any(w.startswith("upgrade") and t0 - 1 <= t <= t0 + 30 for t0, w in disruptive):
            unexplained.append(line.strip())
    print(f"  WebSocket errors not within 30 s after an upgrade: {len(unexplained)}")
    for line in unexplained[:15]:
        print("   ", line)

# Access log and conduit's own log.
lc = os.path.join(out, "logcount.txt")
if os.path.exists(lc):
    lines = sum(int(l.split()[1]) for l in open(lc))
    print(f"\nAccess log: {lines:,} lines over all rotations")
levels = collections.Counter()
panics = []
for line in open(os.path.join(out, "conduit-stderr.log"), errors="replace"):
    parts = line.split()
    if len(parts) > 1 and parts[1] in ("ERROR", "WARN", "INFO", "DEBUG"):
        levels[parts[1]] += 1
    if "panicked" in line:
        panics.append(line.strip())
print(f"conduit log: {dict(levels)}; panics: {len(panics)}")
for p in panics[:5]:
    print("  ", p)
msgs = collections.Counter()
for line in open(os.path.join(out, "conduit-stderr.log"), errors="replace"):
    parts = line.split(None, 3)
    if len(parts) == 4 and parts[1] in ("ERROR", "WARN"):
        msgs[" ".join(parts[1:4])[:110]] += 1
for m, n in msgs.most_common(12):
    print(f"  {n:>7} {m}")
