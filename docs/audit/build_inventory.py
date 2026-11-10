#!/usr/bin/env python3
"""Create a static, local audit inventory and an escaped source browser.

No repository scripts are imported or executed. This is an index, not a semantic auditor.
Only files already tracked by Git are included; new audit artefacts are excluded.
"""
import ast
from collections import Counter
import hashlib
import html
import json
from pathlib import Path
import re
import shutil
import subprocess
import sys
import tomllib
import xml.etree.ElementTree as ET

ROOT = Path(__file__).resolve().parents[2]
OUT = ROOT / "docs/audit"
FINDINGS = {
    "src/proxy.rs": [(291, "F03: host routing membuang port"),
                     (694, "F01: Lookup menerima path yang telah decoded"),
                     (758, "F10: None tidak menghapus header origin"),
                     (1142, "F03: forwarding memakai authority original"),
                     (1343, "F03/F04: identitas key dan konfigurasi")],
    "src/files.rs": [(87, "F09: bitset hanya br/zstd/gzip"),
                     (340, "F01: decode kembali candidate expanded"),
                     (591, "F02: openat2 dan fallback containment"),
                     (632, "F02: check dan open terpisah"),
                     (957, "F02: listing membuka kembali directory melalui path")],
    "src/cache.rs": [(139, "F09: variance content negotiation"),
                     (198, "F03/F04: primary key komponen site/scheme/host/URI"),
                     (401, "F04: retain hanya berdasarkan site masih caching")],
    "src/reload.rs": [(134, "F04: entries site yang masih caching dipertahankan"),
                      (195, "F06: periode nol menuju Tokio interval")],
    "src/units.rs": [(41, "F05: multiplication tanpa checked_mul")],
    "src/limits.rs": [(159, "F07: pembacaan allowance"),
                      (170, "F07: observasi terpisah dari keputusan")],
    "src/tls.rs": [(139, "F08: override key literal"),
                   (173, "F08: lookup concrete host tidak mencari wildcard override")],
    "README.md": [(31, "F10: kontrak off berbeda dari implementasi")],
}
TEST_RE = re.compile(r'#\[(?:tokio::)?test(?:\([^]]*\))?\]\s*'
                     r'(?:#\[[^]]*\]\s*)*(?:async\s+)?fn\s+(\w+)')
SYMBOL_RE = re.compile(r'^\s*(?:(?:pub(?:\([^)]*\))?|unsafe|async|extern\s+"[^"]+")\s+)*'
                       r'(fn|struct|enum|trait|mod|type|const|static)\s+(\w+)')


def run(args):
    p = subprocess.run(args, cwd=ROOT, text=True, capture_output=True)
    return {"command": args, "exit_code": p.returncode,
            "stdout": p.stdout, "stderr": p.stderr}


def category(path):
    if path.startswith("fuzz/seeds/"):
        return "input seed (data, bukan assertion)"
    if path.startswith("graphify-out/"):
        return "artefak graph turunan"
    if path.startswith("assets/"):
        return "aset/dokumentasi brand"
    if path.startswith("src/"):
        return "implementasi Rust / embedded unit tests"
    if path.startswith("tests/"):
        return "integration test dan fixture"
    if path.startswith("scripts/"):
        return "tooling/harness"
    if path.startswith("deploy/"):
        return "deployment/ingestion"
    return "konfigurasi/dokumentasi/build/legal"


def main():
    paths = subprocess.check_output(["git", "ls-files", "-z"], cwd=ROOT).decode().split("\0")
    paths = sorted(p for p in paths if p and not p.startswith("docs/audit/")
                   and (ROOT / p).is_file())
    graph = json.loads((ROOT / "graphify-out/graph.json").read_text())
    nodes, edges = graph["nodes"], graph.get("links", graph.get("edges", []))
    graph_files = Counter(n.get("source_file", "") for n in nodes)
    ids = {n["id"] for n in nodes}
    dangling = sum(e["source"] not in ids or e["target"] not in ids for e in edges)
    degree = Counter()
    for edge in edges:
        degree[edge["source"]] += 1
        degree[edge["target"]] += 1
    by_id = {n["id"]: n for n in nodes}
    records, contents = [], {}
    checks = {"scope": "static syntax and offline models only; no Rust test executed",
              "python": [], "shell": [], "toml": [], "svg": [], "embedded_python": []}
    for name in paths:
        data = (ROOT / name).read_bytes()
        try:
            text = data.decode("utf-8") if b"\0" not in data else None
        except UnicodeDecodeError:
            text = None
        symbols, tests = [], []
        if text is not None:
            for i, line in enumerate(text.splitlines(), 1):
                if name.endswith(".rs") and (m := SYMBOL_RE.match(line)):
                    symbols.append({"line": i, "kind": m[1], "name": m[2]})
            tests = [{"name": m[1], "line": text.count("\n", 0, m.start()) + 1}
                     for m in TEST_RE.finditer(text)] if name.endswith(".rs") else []
            if not name.startswith("graphify-out/"):
                contents[name] = text
        records.append({"path": name, "category": category(name), "bytes": len(data),
                        "lines": len(text.splitlines()) if text is not None else None,
                        "sha256": hashlib.sha256(data).hexdigest(), "text": text is not None,
                        "graph_nodes": graph_files[name], "symbols": symbols, "tests": tests,
                        "finding_markers": FINDINGS.get(name, [])})
        if name.endswith(".py") and text is not None:
            try:
                ast.parse(text, filename=name)
                checks["python"].append({"path": name, "status": "parsed"})
            except SyntaxError as e:
                checks["python"].append({"path": name, "status": "failed", "error": str(e)})
        if name.endswith(".sh"):
            checks["shell"].append(run(["bash", "-n", name]))
            if text is not None:
                lines = text.splitlines()
                i = 0
                while i < len(lines):
                    match = re.search(r"<<'PY'", lines[i])
                    if match:
                        end = i + 1
                        while end < len(lines) and lines[end] != "PY":
                            end += 1
                        try:
                            ast.parse("\n".join(lines[i + 1:end]), filename=name + ":heredoc")
                            status = "parsed"
                        except SyntaxError as error:
                            status = str(error)
                        checks["embedded_python"].append({"path": name, "start_line": i + 2,
                                                           "status": status})
                        i = end
                    i += 1
        if name.endswith(".toml") and text is not None:
            try:
                tomllib.loads(text)
                checks["toml"].append({"path": name, "status": "parsed"})
            except tomllib.TOMLDecodeError as e:
                checks["toml"].append({"path": name, "status": "failed", "error": str(e)})
        if name.endswith(".svg"):
            try:
                ET.fromstring(data)
                checks["svg"].append({"path": name, "status": "parsed XML"})
            except ET.ParseError as e:
                checks["svg"].append({"path": name, "status": "failed", "error": str(e)})
    for path in OUT.glob("*.py"):
        ast.parse(path.read_text(), filename=str(path))
    checks["environment"] = {"cargo": shutil.which("cargo"), "rustc": shutil.which("rustc"),
                             "vendor_manifest_exists": (ROOT / "vendor/pingora/Cargo.toml").exists(),
                             "submodule": run(["git", "submodule", "status"])}
    checks["graph"] = {"dangling_edges": dangling, "duplicate_ids": len(nodes) - len(ids),
                       "empty_source_file_nodes": graph_files[""],
                       "confidence": dict(Counter(e.get("confidence") for e in edges))}
    result = run([sys.executable, str(OUT / "reproduce.py")])
    if result["exit_code"]:
        raise RuntimeError(result)
    (OUT / "reproduction-results.json").write_text(result["stdout"])
    checks["offline_demonstrations"] = {"exit_code": result["exit_code"],
                                       "count": len(json.loads(result["stdout"])["demonstrations"]),
                                       "rust_binary_executed": False}
    graph_stats = {"nodes": len(nodes), "edges": len(edges),
                   "communities": len({n.get("community") for n in nodes}),
                   "top_degree": [{"label": by_id[n].get("label"), "degree": count,
                                   "source_file": by_id[n].get("source_file")}
                                  for n, count in degree.most_common(12)]}
    inventory = {"date": "2026-11-10", "git_head": run(["git", "rev-parse", "HEAD"])["stdout"].strip(),
                 "scope_note": "Tracked regular files present; audit outputs excluded; symbols indexed lexically, not a Rust AST",
                 "file_count": len(records), "graph": graph_stats, "files": records}
    (OUT / "inventory.json").write_text(json.dumps(inventory, indent=2, ensure_ascii=False) + "\n")
    (OUT / "checks.json").write_text(json.dumps(checks, indent=2, ensure_ascii=False) + "\n")
    md = ["# Inventaris file dan simbol", "",
          "Indeks otomatis, bukan klaim review semantik perbaris. Semua file tracked yang tersedia dicatat; vendor gitlink kosong tidak menjadi file reguler. Hash lengkap di inventory.json.", "",
          "| File | Baris | Byte | Node graph | Jenis |", "|---|---:|---:|---:|---|"]
    for record in records:
        name = record["path"]
        md.append(f"| [{name}](../../{name}) | {record['lines'] if record['text'] else 'biner'} | {record['bytes']} | {record['graph_nodes']} | {record['category']} |")
    md += ["", "## Simbol Rust dan test", "",
           "Deklarasi dideteksi regex perbaris; macro, signature multiline dan nama metode sama tidak diselesaikan sebagai AST. Posisi adalah titik navigasi, bukan bukti call graph.", ""]
    for record in records:
        if not record["symbols"]:
            continue
        md += [f"### {record['path']}", "",
               f"{record['lines']} baris; {len(record['tests'])} deklarasi test ditemukan.", "",
               "| Baris | Jenis | Simbol |", "|---:|---|---|"]
        for symbol in record["symbols"]:
            md.append(f"| {symbol['line']} | {symbol['kind']} | `{symbol['name']}` |")
        md.append("")
    (OUT / "INVENTORY.md").write_text("\n".join(md) + "\n")
    make_browser(records, contents)
    print(json.dumps({"files": len(records), "source_browser_files": len(contents),
                      "rust_src_lines": sum(r["lines"] for r in records if r["path"].startswith("src/") and r["path"].endswith(".rs")),
                      "unit_test_declarations": sum(len(r["tests"]) for r in records if r["path"].startswith("src/")),
                      "e2e_test_declarations": sum(len(r["tests"]) for r in records if r["path"].startswith("tests/")),
                      "python_parsed": len(checks["python"]), "shell_checked": len(checks["shell"]),
                      "toml_parsed": len(checks["toml"]), "svg_parsed": len(checks["svg"]),
                      "offline_models": checks["offline_demonstrations"]["count"], "graph": checks["graph"]}, indent=2))


def make_browser(records, contents):
    e = html.escape
    chunks = ['<!doctype html><html lang="id"><meta charset="utf-8">',
              '<meta name="viewport" content="width=device-width,initial-scale=1">',
              '<title>Conduit — penjelajah audit sumber</title>',
              '<style>body{font:15px system-ui;margin:0;background:#faf8f5;color:#222}header{padding:20px;background:#24211e;color:white}a{color:#a33b10}header a{color:#ffd0b5}nav{position:sticky;top:0;background:#eee7df;padding:12px;z-index:1}input{padding:8px;min-width:260px}main{padding:20px}details{border:1px solid #ddd;margin:8px 0;background:white}summary{padding:12px;cursor:pointer;overflow-wrap:anywhere}table{border-collapse:collapse;width:100%}td{vertical-align:top;padding:0 8px}.num{width:5ch;text-align:right;background:#f2eee9;user-select:none}.num a{text-decoration:none;color:#777}code{font:12px/1.6 ui-monospace,monospace;white-space:pre-wrap;overflow-wrap:anywhere}tr:target{background:#ffe4b6}.note{background:#fff0d5;border-left:5px solid #c54c17;padding:12px}small{color:#666}.meta{padding:12px}.hidden{display:none}</style>',
              '<header><h1>Conduit: sumber per file dan baris</h1><p>Snapshot 10 November 2026. Nomor baris dan marker temuan untuk penelusuran; bukan bukti test atau review semantik setiap baris.</p><p><a href="README.md">Laporan utama</a> · <a href="FINDINGS.md">Temuan</a> · <a href="WALKTHROUGH.md">Bedah modul</a></p></header>',
              '<nav><label>Filter nama file: <input id="filter" type="search" placeholder="misalnya src/cache.rs"></label> <span id="count"></span><p>Gunakan pencarian browser untuk teks. Source berupa teks yang di-escape; script proyek tidak dijalankan.</p></nav><main>']
    for record in records:
        name = record["path"]
        fid = "file-" + name.replace("/", "--").replace(".", "_")
        chunks.append(f'<details data-path="{e(name, quote=True)}" id="{fid}"><summary>{e(name)} · {record["lines"] if record["text"] else "biner"} baris · {record["bytes"]:,} byte</summary>')
        chunks.append(f'<div class="meta"><small>{e(record["category"])} · SHA256 {record["sha256"]} · {record["graph_nodes"]} node graph</small></div>')
        if name in contents:
            markers = dict(FINDINGS.get(name, []))
            chunks.append('<table>')
            for i, line in enumerate(contents[name].splitlines(), 1):
                if i in markers:
                    chunks.append(f'<tr><td colspan="2" class="note">{e(markers[i])} — lihat FINDINGS.md untuk prasyarat dan batas bukti.</td></tr>')
                lid = f'{fid}-L{i}'
                chunks.append(f'<tr id="{lid}"><td class="num"><a href="#{lid}">{i}</a></td><td><code>{e(line) if line else " "}</code></td></tr>')
            chunks.append('</table>')
        elif name.startswith("graphify-out/"):
            chunks.append('<p class="meta">Artefak turunan besar: dicatat hash/ukuran, isi tidak disalin ke penjelajah. Gunakan graph.html atau graph.json aslinya.</p>')
        else:
            chunks.append('<p class="meta">File biner: tidak diperlakukan sebagai kode atau klaim review visual.</p>')
        chunks.append('</details>')
    chunks += ['</main><script>const files=[...document.querySelectorAll("details[data-path]")];const input=document.getElementById("filter");function apply(){let n=0;for(const f of files){const show=f.dataset.path.toLowerCase().includes(input.value.toLowerCase());f.classList.toggle("hidden",!show);if(show)n++;}document.getElementById("count").textContent=n+" file";}input.addEventListener("input",apply);function reveal(){if(!location.hash)return;const el=document.getElementById(decodeURIComponent(location.hash.slice(1)));if(el){const d=el.closest("details");if(d){d.open=true;d.classList.remove("hidden");}requestAnimationFrame(()=>el.scrollIntoView());}}window.addEventListener("hashchange",reveal);apply();reveal();</script></html>']
    (OUT / "SOURCE.html").write_text("\n".join(chunks))


if __name__ == "__main__":
    main()
