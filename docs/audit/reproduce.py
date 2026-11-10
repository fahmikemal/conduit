#!/usr/bin/env python3
"""Offline audit demonstrations. Models, NOT tests of the Rust binary.

Run from any directory: python3 docs/audit/reproduce.py
Only temporary files are created; no server, network, or credentials are used.
"""
from pathlib import Path
from tempfile import TemporaryDirectory
from urllib.parse import unquote_to_bytes
import json

ROOT = Path(__file__).resolve().parents[2]


def normalize(raw):
    """Translation of pathnorm.rs for the selected ASCII examples only."""
    decoded = unquote_to_bytes(raw)
    assert b"\0" not in decoded
    segments = []
    trailing = decoded.endswith(b"/")
    for segment in decoded.split(b"/"):
        if segment == b"":
            continue
        if segment == b".":
            trailing = True
        elif segment == b"..":
            if not segments:
                raise ValueError("above root")
            segments.pop()
            trailing = True
        else:
            segments.append(segment)
            trailing = False
    trailing |= decoded.endswith(b"/")
    out = b"/" + b"/".join(segments)
    if segments and trailing:
        out += b"/"
    return out


def accepted_bits(value):
    """Relevant subset of files::Accepted for well-formed token-only examples."""
    bits = {"br": 1, "zstd": 2, "gzip": 4, "x-gzip": 4}
    out = 0
    for token in value.split(","):
        out |= bits.get(token.strip().lower(), 0)
    return out


def main():
    results = []
    proxy = (ROOT / "src/proxy.rs").read_text()
    files = (ROOT / "src/files.rs").read_text()
    assert "let Ok(path) = pathnorm::normalize(raw_path)" in proxy
    assert "let Ok(target) = pathnorm::normalize(&expanded)" in files
    raw = b"/%2561dmin/secret.txt"
    checked = normalize(raw)
    opened = normalize(checked)
    assert not checked.startswith(b"/admin")
    assert opened == b"/admin/secret.txt"
    results.append({"finding": "F01", "kind": "algorithm model", "raw": raw.decode(),
                    "plugin_path": checked.decode(), "file_path": opened.decode()})

    # Force the interleaving between canonicalize/check and open. Both roots are disposable.
    with TemporaryDirectory(prefix="conduit-audit-") as tmp:
        base = Path(tmp)
        root = base / "root"
        outside = base / "outside"
        root.mkdir()
        outside.mkdir()
        (root / "item").write_text("inside")
        (outside / "item").write_text("outside-marker")
        checked_path = (root / "item").resolve()
        assert checked_path.is_relative_to(root.resolve())
        (root / "item").unlink()
        (root / "item").symlink_to(outside / "item")
        observed = checked_path.read_text()
        assert observed == "outside-marker"
        results.append({"finding": "F02", "kind": "real filesystem interleaving",
                        "result": observed, "scope": "temporary directories only"})

    # request_host removes the port, while original_host forwards it.
    hosts = ["app.test:8080", "app.test:9090"]
    keys = [h.rsplit(":", 1)[0] for h in hosts]
    assert keys[0] == keys[1] and hosts[0] != hosts[1]
    results.append({"finding": "F03", "kind": "key-input model",
                    "upstream_hosts": hosts, "cache_hosts": keys})

    n = 9223372036854775808
    wrapped = (n * 60) % (1 << 64)
    assert wrapped == 0
    results.append({"finding": "F05", "kind": "unsigned arithmetic model",
                    "input": f"{n}m", "release_wrapped_seconds": wrapped,
                    "debug": "overflow panic with overflow checks enabled"})

    # Both workers see the same count before either observes its accepted request.
    limit, used = 1, 0
    admissions = [used + 1 <= limit for _ in range(2)]
    assert sum(admissions) > limit
    results.append({"finding": "F07", "kind": "concurrent interleaving model",
                    "limit": limit, "admitted": sum(admissions)})

    overrides = {"*.example.test": "preferred-cert"}
    wildcard_san = {"example.test": "first-configured-cert"}
    host = "app.example.test"
    chosen = overrides.get(host) or wildcard_san.get(host.split(".", 1)[1])
    assert chosen != "preferred-cert"
    results.append({"finding": "F08", "kind": "certificate selection model",
                    "selected": chosen, "configured_override": "preferred-cert"})

    assert accepted_bits("deflate") == accepted_bits("identity") == 0
    results.append({"finding": "F09", "kind": "variance-input model",
                    "requests": ["deflate", "identity"], "bits": [0, 0],
                    "condition": "upstream Vary: Accept-Encoding takes the listed branch"})

    print(json.dumps({"warning": "Models and one filesystem demonstration; Rust not executed",
                      "demonstrations": results}, indent=2))


if __name__ == "__main__":
    main()
