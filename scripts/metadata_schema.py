#!/usr/bin/env python3
"""Observe the structure of 1C Designer XML dumps without assuming a schema.

Offline research tool, not part of the language server. Every element path is
keyed by the chain of expanded QNames (namespace URI + local name) from the
document root. For each path the scanner counts occurrences, documents and
sources, the child elements and attributes seen under it with their observed
multiplicity, and the shape of leaf values.

Nothing here is a schema: a value domain is the set of values met in the
corpus, "present in every document" is not minOccurs=1, and an unseen value is
not a forbidden one.

Private configurations are never copied. The published aggregate keeps only
structure, counts and the values that pass `publishable_values`; full value
sets and the per-file manifest go to `--local-out`, outside the repository.

Run from the repository root with Python 3.11+:

  python3 scripts/metadata_schema.py scan \
      --source lombard/cf=cf:/path/to/src/cf \
      --extensions lombard=/path/to/src/cfe \
      --source fixtures/fixtures=fixture:crates/bsl-metadata/fixtures \
      --out docs/legal/bsl-metadata --local-out /some/private/dir

  python3 scripts/metadata_schema.py crosscheck-mdo \
      --aggregate docs/legal/bsl-metadata/aggregate.json --mdo-root /path/to/edt/src
"""

from __future__ import annotations

import argparse
from collections import Counter
from concurrent.futures import ProcessPoolExecutor
import hashlib
import io
import json
from pathlib import Path
import re
import subprocess
import sys
import xml.etree.ElementTree as ET
import xml.parsers.expat

MD_CLASSES = "http://v8.1c.ru/8.3/MDClasses"
XML_NS = "http://www.w3.org/XML/1998/namespace"

# A path keeps every distinct value up to this many; past it the domain is
# treated as open and values are no longer collected.
VALUE_CAP = 64
MAX_VALUE_LEN = 80
# A token or text value from a private source is published only when this many
# independent projects show it, so an object name used in one project never
# leaves it. Booleans, numbers and empty values carry no names and are shown.
MIN_PROJECTS = 2
NAMELESS_KINDS = ("empty", "bool", "int", "decimal")
# How many times one QName may repeat on a published path, see `fold`.
FOLD_LIMIT = 2
# Paths seen in fewer documents carry a witness locator.
RARE_DOCS = 3

SOURCE_KINDS = ("cf", "cfe", "fixture")

UUID_RE = re.compile(r"^[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}$")
INT_RE = re.compile(r"^-?\d+$")
DECIMAL_RE = re.compile(r"^-?\d+\.\d+$")
TOKEN_RE = re.compile(r"^[\w.:#/-]+$")


def value_kind(text: str) -> str:
    if text == "":
        return "empty"
    if text in ("true", "false"):
        return "bool"
    if INT_RE.match(text):
        return "int"
    if DECIMAL_RE.match(text):
        return "decimal"
    if UUID_RE.match(text):
        return "uuid"
    if TOKEN_RE.match(text):
        return "token"
    return "text"


def collectable(text: str) -> bool:
    return len(text) <= MAX_VALUE_LEN and "\n" not in text and value_kind(text) != "uuid"


class ValueStats:
    """Kinds of a value position plus, while the domain stays small, its values."""

    __slots__ = ("kinds", "values")

    def __init__(self):
        self.kinds: Counter = Counter()
        # value -> Counter(source id -> occurrences); None once the domain is open
        self.values: dict | None = {}

    def add(self, text: str, source: str):
        self.kinds[value_kind(text)] += 1
        if self.values is None or not collectable(text):
            return
        self.values.setdefault(text, Counter())[source] += 1
        if len(self.values) > VALUE_CAP:
            self.values = None

    def merge(self, other: "ValueStats"):
        self.kinds.update(other.kinds)
        if self.values is None or other.values is None:
            self.values = None
            return
        for value, sources in other.values.items():
            self.values.setdefault(value, Counter()).update(sources)
        if len(self.values) > VALUE_CAP:
            self.values = None


class PathStats:
    __slots__ = ("occ", "docs", "sources", "source_docs", "versions", "attrs", "kids", "text", "witness", "_doc")

    def __init__(self):
        self.occ = 0
        self.docs = 0
        self.sources: Counter = Counter()
        self.source_docs: Counter = Counter()
        self.versions: Counter = Counter()
        self.attrs: dict[str, ValueStats] = {}
        # child QName -> [parents having it, min per parent when present, max per parent]
        self.kids: dict[str, list[int]] = {}
        self.text: ValueStats | None = None
        self.witness: tuple[str, str] | None = None
        self._doc = -1

    def enter(self, doc_id: int, source: str, rel: str, version: str, attrib):
        self.occ += 1
        self.sources[source] += 1
        self.versions[version] += 1
        if self._doc != doc_id:
            self._doc = doc_id
            self.docs += 1
            self.source_docs[source] += 1
        if self.witness is None or (source, rel) < self.witness:
            self.witness = (source, rel)
        for name, value in attrib.items():
            self.attrs.setdefault(name, ValueStats()).add(value, source)

    def leave(self, counts: Counter, text: str | None, source: str):
        for name, count in counts.items():
            kid = self.kids.get(name)
            if kid is None:
                self.kids[name] = [1, count, count]
            else:
                kid[0] += 1
                kid[1] = min(kid[1], count)
                kid[2] = max(kid[2], count)
        if text is not None:
            if self.text is None:
                self.text = ValueStats()
            self.text.add(text, source)

    def merge(self, other: "PathStats"):
        self.occ += other.occ
        self.docs += other.docs
        self.sources.update(other.sources)
        self.source_docs.update(other.source_docs)
        self.versions.update(other.versions)
        for name, stats in other.attrs.items():
            self.attrs.setdefault(name, ValueStats()).merge(stats)
        for name, (parents, low, high) in other.kids.items():
            mine = self.kids.get(name)
            if mine is None:
                self.kids[name] = [parents, low, high]
            else:
                mine[0] += parents
                mine[1] = min(mine[1], low)
                mine[2] = max(mine[2], high)
        if other.text is not None:
            if self.text is None:
                self.text = ValueStats()
            self.text.merge(other.text)
        if other.witness is not None and (self.witness is None or other.witness < self.witness):
            self.witness = other.witness


class Observation:
    """Mergeable result of scanning a set of documents."""

    def __init__(self):
        self.paths: dict[tuple[str, ...], PathStats] = {}
        # The same observations keyed by `fold`ed paths; this is what is published.
        self.folded: dict[tuple[str, ...], PathStats] = {}
        self.files: list[dict] = []
        self.errors: list[dict] = []
        # (source, kind local name, top directory) -> documents
        self.layout: Counter = Counter()
        # (source, kind local name) -> Configuration.xml ChildObjects entries
        self.declared: Counter = Counter()

    def merge(self, other: "Observation"):
        for table, theirs in ((self.paths, other.paths), (self.folded, other.folded)):
            for key, stats in theirs.items():
                mine = table.get(key)
                if mine is None:
                    table[key] = stats
                else:
                    mine.merge(stats)
        self.files.extend(other.files)
        self.errors.extend(other.errors)
        self.layout.update(other.layout)
        self.declared.update(other.declared)

    def scan_document(self, doc_id: int, source: str, rel: str, data: bytes):
        digest = hashlib.sha256(data).hexdigest()
        # Well-formedness is checked before anything is counted, so a broken
        # document is only an error record and never a partial observation.
        try:
            # A namespace-aware parser, so an unbound prefix is an error here too.
            xml.parsers.expat.ParserCreate(namespace_separator="}").Parse(data, True)
        except xml.parsers.expat.ExpatError as err:
            self.errors.append({"source": source, "path": rel, "sha256": digest, "error": str(err)})
            self.files.append({"source": source, "path": rel, "sha256": digest, "root": None, "version": None})
            return
        self._walk(doc_id, source, rel, data)
        self.files.append({"source": source, "path": rel, "sha256": digest, "root": self._root, "version": self._version})

    def _walk(self, doc_id: int, source: str, rel: str, data: bytes):
        self._root = None
        self._version = None
        stack: list[tuple[tuple[str, ...], tuple[str, ...], Counter]] = []
        version = "-"
        for event, elem in ET.iterparse(io.BytesIO(data), events=("start", "end")):
            if event == "start":
                if not stack:
                    version = elem.get("version", "-")
                    self._root = elem.tag
                    self._version = version
                    path = folded = (elem.tag,)
                else:
                    parent_path, parent_folded, counts = stack[-1]
                    counts[elem.tag] += 1
                    path = parent_path + (elem.tag,)
                    folded = fold(parent_folded, elem.tag)
                for table, key in ((self.paths, path), (self.folded, folded)):
                    stats = table.get(key)
                    if stats is None:
                        stats = table[key] = PathStats()
                    stats.enter(doc_id, source, rel, version, elem.attrib)
                stack.append((path, folded, Counter()))
                continue
            path, folded, counts = stack.pop()
            text = None if counts else (elem.text or "").strip()
            self.paths[path].leave(counts, text, source)
            self.folded[folded].leave(counts, text, source)
            self._layout(source, rel, path, elem)
            elem.clear()

    def _layout(self, source: str, rel: str, path: tuple[str, ...], elem):
        if len(path) == 2 and path[0] == f"{{{MD_CLASSES}}}MetaDataObject":
            kind = local(path[1])
            top = rel.split("/", 1)[0] if "/" in rel else "."
            self.layout[(source, kind, top)] += 1
        if (
            len(path) == 4
            and rel == "Configuration.xml"
            and path[1] == f"{{{MD_CLASSES}}}Configuration"
            and path[2] == f"{{{MD_CLASSES}}}ChildObjects"
        ):
            self.declared[(source, local(path[3]))] += 1


def fold(parent: tuple[str, ...], tag: str) -> tuple[str, ...]:
    """Key of `tag` under the folded `parent` path.

    Forms nest groups in groups to any depth, so full paths of a recursive
    element grow without bound and say nothing new past the second level. A
    QName met for the third time on a path is folded onto its second occurrence:
    `a/b/x/b/y/b/z` is keyed `a/b/x/b/z`. Two occurrences stay distinct, so
    `Attribute` of an object and of its tabular section keep separate keys.
    """
    if parent.count(tag) < FOLD_LIMIT:
        return parent + (tag,)
    return parent[: len(parent) - parent[::-1].index(tag)]


def local(qname: str) -> str:
    return qname.rsplit("}", 1)[-1]


def namespace(qname: str) -> str:
    return qname[1:].split("}", 1)[0] if qname.startswith("{") else ""


def scan_chunk(chunk: list[tuple[int, str, str, str]]) -> Observation:
    observation = Observation()
    for doc_id, source, root, rel in chunk:
        observation.scan_document(doc_id, source, rel, (Path(root) / rel).read_bytes())
    for stats in [*observation.paths.values(), *observation.folded.values()]:
        stats._doc = -1
    return observation


class Source:
    def __init__(self, project: str, ident: str, kind: str, root: Path, label: str | None = None):
        if kind not in SOURCE_KINDS:
            raise SystemExit(f"unknown source kind {kind!r}, expected one of {SOURCE_KINDS}")
        self.project = project
        self.id = f"{project}/{ident}"
        self.kind = kind
        self.root = root
        self.label = label or ident

    def documents(self, skip: set[Path] = frozenset()) -> list[str]:
        files = []
        for path in self.root.rglob("*.xml"):
            if any(path.resolve().is_relative_to(other) for other in skip):
                continue
            files.append(path.relative_to(self.root).as_posix())
        return sorted(files)


def parse_source(spec: str) -> Source:
    name, _, rest = spec.partition("=")
    kind, _, root = rest.partition(":")
    project, _, ident = name.partition("/")
    if not (project and ident and kind and root):
        raise SystemExit(f"bad --source {spec!r}, expected PROJECT/ID=KIND:PATH")
    return Source(project, ident, kind, Path(root))


def expand_extensions(spec: str) -> tuple[list[Source], Path]:
    project, _, root = spec.partition("=")
    if not (project and root):
        raise SystemExit(f"bad --extensions {spec!r}, expected PROJECT=PATH")
    base = Path(root)
    dirs = sorted(p.parent for p in base.glob("*/Configuration.xml"))
    return [Source(project, f"cfe-{i}", "cfe", d, d.name) for i, d in enumerate(dirs, 1)], base


def revision(root: Path) -> dict:
    try:
        top = subprocess.check_output(["git", "-C", str(root), "rev-parse", "HEAD"], text=True, stderr=subprocess.DEVNULL).strip()
        dirty = subprocess.check_output(
            ["git", "-C", str(root), "status", "--porcelain", "--", "."], text=True, stderr=subprocess.DEVNULL
        )
        return {"git": top, "dirty_entries": len(dirty.splitlines())}
    except (subprocess.CalledProcessError, FileNotFoundError):
        return {"git": None}


def scan(sources: list[Source], extension_bases: list[Path], jobs: int) -> tuple[Observation, list[dict]]:
    jobs_list = []
    unassigned = []
    claimed = {s.root.resolve() for s in sources}
    for base in extension_bases:
        for path in sorted(base.rglob("*.xml")):
            if not any(path.resolve().is_relative_to(c) for c in claimed):
                unassigned.append(path)
    for source in sources:
        nested = {s.root.resolve() for s in sources if s is not source and s.root.resolve().is_relative_to(source.root.resolve())}
        for rel in source.documents(nested):
            jobs_list.append((len(jobs_list), source.id, str(source.root), rel))
    size = max(1, len(jobs_list) // (jobs * 8) or 1)
    chunks = [jobs_list[i : i + size] for i in range(0, len(jobs_list), size)]
    total = Observation()
    if jobs <= 1:
        for chunk in chunks:
            total.merge(scan_chunk(chunk))
    else:
        with ProcessPoolExecutor(jobs) as pool:
            for part in pool.map(scan_chunk, chunks):
                total.merge(part)
    total.files.sort(key=lambda f: (f["source"], f["path"]))
    total.errors.sort(key=lambda f: (f["source"], f["path"]))
    return total, [{"path": str(p)} for p in unassigned]


def public_values(table: dict, projects: dict[str, str], kinds: dict[str, str]) -> set[str]:
    """Values that may leave the machine.

    A value is public when it names nothing (boolean, number, empty), occurs in
    a fixture of this repository, or occurs in at least `MIN_PROJECTS` private
    projects anywhere in the corpus. Support is counted per value, not per path:
    a format token such as an enumeration member is shared by projects even
    where one path of it is seen in a single project.
    """
    public: set[str] = set()
    support: dict[str, set[str]] = {}
    for stats in table.values():
        for values in [a.values for a in stats.attrs.values()] + [stats.text.values if stats.text else None]:
            for value, per_source in (values or {}).items():
                if value_kind(value) in NAMELESS_KINDS or any(kinds[s] == "fixture" for s in per_source):
                    public.add(value)
                else:
                    support.setdefault(value, set()).update(projects[s] for s in per_source)
    return public | {value for value, seen in support.items() if len(seen) >= MIN_PROJECTS}


def value_report(stats: ValueStats, public: set[str]) -> dict:
    out = {"kinds": dict(sorted(stats.kinds.items()))}
    if stats.values is None:
        out["domain"] = "open"
        return out
    out["domain"] = f"observed<={VALUE_CAP}"
    out["values"] = {
        value: {"occ": sum(per_source.values()), "sources": len(per_source)}
        for value, per_source in sorted(stats.values.items())
        if value in public
    }
    withheld = sum(value not in public for value in stats.values)
    if withheld:
        out["withheld"] = withheld
    return out


class Prefixes:
    def __init__(self, table: dict):
        uris = sorted({namespace(q) for key in table for q in key})
        uris += sorted({namespace(a) for s in table.values() for a in s.attrs} - set(uris))
        self.map = {uri: f"n{i}" for i, uri in enumerate(uris)}

    def q(self, qname: str) -> str:
        ns = namespace(qname)
        return f"{self.map[ns]}:{local(qname)}" if ns else local(qname)


def locator(witness: tuple[str, str], kinds: dict[str, str], digests: dict | None) -> list[str]:
    """A witness of a private source is named by its file hash, not its path:
    paths of private dumps carry object names. `manifest.local.json` maps the
    hash back to the file. Fixtures are in this repository and keep the path."""
    source, rel = witness
    if digests is None or kinds[source] == "fixture":
        return [source, rel]
    return [source, f"sha256:{digests[(source, rel)]}"]


def aggregate(table: dict, sources: list[Source], digests: dict | None) -> dict:
    """`digests` maps (source, path) to the file hash; None keeps raw paths
    for the local, unpublished aggregate."""
    projects = {s.id: s.project for s in sources}
    kinds = {s.id: s.kind for s in sources}
    prefixes = Prefixes(table)
    public = public_values(table, projects, kinds)
    paths = {}
    for key in sorted(table, key=lambda k: [prefixes.q(q) for q in k]):
        stats = table[key]
        entry = {
            "occ": stats.occ,
            "docs": stats.docs,
            "sources": dict(sorted(stats.sources.items())),
            "versions": dict(sorted(stats.versions.items())),
        }
        if stats.attrs:
            entry["attributes"] = {prefixes.q(a): value_report(v, public) for a, v in sorted(stats.attrs.items(), key=lambda i: prefixes.q(i[0]))}
        if stats.kids:
            entry["children"] = {
                prefixes.q(c): {"parents": p, "parents_without": stats.occ - p, "min": lo, "max": hi}
                for c, (p, lo, hi) in sorted(stats.kids.items(), key=lambda i: prefixes.q(i[0]))
            }
        if stats.text is not None:
            entry["text"] = value_report(stats.text, public)
        if stats.docs < RARE_DOCS and stats.witness is not None:
            entry["witness"] = locator(stats.witness, kinds, digests)
        paths["/".join(prefixes.q(q) for q in key)] = entry
    return {
        "namespaces": {prefix: uri for uri, prefix in sorted(prefixes.map.items(), key=lambda i: int(i[1][1:]))},
        "rules": {
            "value_cap": VALUE_CAP,
            "max_value_len": MAX_VALUE_LEN,
            "min_projects": MIN_PROJECTS,
            "rare_docs": RARE_DOCS,
            "fold_limit": FOLD_LIMIT,
        },
        "paths": paths,
    }


def layout_report(observation: Observation) -> dict:
    kinds: dict[str, dict] = {}
    for (source, kind, top), docs in sorted(observation.layout.items()):
        entry = kinds.setdefault(kind, {"directories": {}, "declared_in": {}})
        entry["directories"].setdefault(top, {})[source] = docs
    for (source, kind), count in sorted(observation.declared.items()):
        kinds.setdefault(kind, {"directories": {}, "declared_in": {}})["declared_in"][source] = count
    return dict(sorted(kinds.items()))


def files_digest(files: list[dict]) -> str:
    digest = hashlib.sha256()
    for f in files:
        digest.update(f"{f['path']}\0{f['sha256']}\n".encode())
    return digest.hexdigest()


def manifest(observation: Observation, sources: list[Source], unassigned: list[dict], private: bool) -> dict:
    out = []
    for source in sources:
        files = [f for f in observation.files if f["source"] == source.id]
        roots = Counter((local(f["root"]) if f["root"] else "!error", f["version"] or "-") for f in files)
        entry = {
            "id": source.id,
            "kind": source.kind,
            "project": source.project,
            "revision": revision(source.root) if source.kind != "fixture" else {"repository": "this"},
            "documents": len(files),
            "files_sha256": files_digest(files),
            "roots": {f"{r} version={v}": n for (r, v), n in sorted(roots.items())},
            "parse_errors": [
                e if private or source.kind == "fixture" else {k: v for k, v in e.items() if k != "path"}
                for e in observation.errors
                if e["source"] == source.id
            ],
        }
        if private or source.kind == "fixture":
            entry["locator"] = str(source.root)
            entry["label"] = source.label
            entry["files"] = [{"path": f["path"], "sha256": f["sha256"]} for f in files]
        out.append(entry)
    return {"sources": out, "unassigned_xml": len(unassigned)}


def write_json(path: Path, data):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(data, ensure_ascii=False, indent=1, sort_keys=False) + "\n", encoding="utf-8")


def write_aggregate(path: Path, data: dict):
    """One path per line: the file stays diffable and an order of magnitude smaller."""
    path.parent.mkdir(parents=True, exist_ok=True)
    compact = dict(ensure_ascii=False, separators=(",", ":"))
    lines = ["{"]
    for key in ("namespaces", "rules"):
        lines.append(f"{json.dumps(key)}:{json.dumps(data[key], **compact)},")
    lines.append('"paths":{')
    entries = list(data["paths"].items())
    for i, (key, entry) in enumerate(entries):
        tail = "," if i + 1 < len(entries) else ""
        lines.append(f"{json.dumps(key, ensure_ascii=False)}:{json.dumps(entry, **compact)}{tail}")
    lines.append("}}")
    path.write_text("\n".join(lines) + "\n", encoding="utf-8")


def cmd_scan(args):
    sources = [parse_source(s) for s in args.source]
    bases = []
    for spec in args.extensions:
        expanded, base = expand_extensions(spec)
        sources += expanded
        bases.append(base)
    ids = [s.id for s in sources]
    if len(ids) != len(set(ids)):
        raise SystemExit(f"duplicate source ids in {ids}")
    roots = [s.root.resolve() for s in sources]
    if len(roots) != len(set(roots)):
        raise SystemExit("two sources share one root directory")
    observation, unassigned = scan(sources, bases, args.jobs)
    out = Path(args.out)
    write_json(out / "manifest.json", manifest(observation, sources, unassigned, private=False))
    digests = {(f["source"], f["path"]): f["sha256"] for f in observation.files}
    write_aggregate(out / "aggregate.json", aggregate(observation.folded, sources, digests))
    write_json(out / "layout.json", layout_report(observation))
    if args.local_out:
        local_dir = Path(args.local_out)
        write_json(local_dir / "manifest.local.json", manifest(observation, sources, unassigned, private=True))
        write_aggregate(local_dir / "aggregate.full.local.json", aggregate(observation.paths, sources, None))
        values = {
            "/".join(key): {
                "text": stats.text.values if stats.text and stats.text.values is not None else None,
                "attributes": {a: v.values for a, v in stats.attrs.items() if v.values is not None},
            }
            for key, stats in sorted(observation.paths.items())
        }
        write_json(local_dir / "values.local.json", values)
    print(f"sources={len(sources)} documents={len(observation.files)} paths={len(observation.paths)} published_paths={len(observation.folded)} errors={len(observation.errors)}")


def cmd_crosscheck(args):
    """Compare local names and published values with an EDT `.mdo` export.

    Only names and values are compared; the `.mdo` structure is not read as a
    model of the format. EDT spells element names in lowerCamelCase, so names
    are matched ignoring case.
    """
    data = json.loads(Path(args.aggregate).read_text(encoding="utf-8"))
    xml_values: dict[str, set[str]] = {}
    xml_withheld: Counter = Counter()
    xml_names: set[str] = set()
    for key, entry in data["paths"].items():
        name = key.rsplit("/", 1)[-1].split(":")[-1].lower()
        xml_names.add(name)
        if "values" in entry.get("text", {}):
            xml_values.setdefault(name, set()).update(entry["text"]["values"])
            xml_withheld[name] += entry["text"].get("withheld", 0)
    mdo_values: dict[str, set[str]] = {}
    mdo_names: set[str] = set()
    files = sorted(Path(args.mdo_root).rglob("*.mdo"))
    for path in files:
        for _, elem in ET.iterparse(path, events=("end",)):
            name = local(elem.tag).lower()
            mdo_names.add(name)
            if len(elem) == 0 and name in xml_values:
                text = (elem.text or "").strip()
                if collectable(text):
                    mdo_values.setdefault(name, set()).add(text)
            elem.clear()
    report = {
        "mdo_files": len(files),
        "shared_names": len(xml_names & mdo_names),
        "note": "XML values are the published ones; mdo_only_count may include values withheld from the "
        "published aggregate, xml_withheld counts them per name",
        "values": {},
    }
    for name in sorted(xml_values):
        if name not in mdo_values:
            continue
        both = xml_values[name] & mdo_values[name]
        report["values"][name] = {
            "both": sorted(both),
            "xml_only": sorted(xml_values[name] - mdo_values[name]),
            "mdo_only_count": len(mdo_values[name] - xml_values[name]),
            "xml_withheld": xml_withheld[name],
        }
    text = json.dumps(report, ensure_ascii=False, indent=1) + "\n"
    if args.out:
        Path(args.out).write_text(text, encoding="utf-8")
    else:
        sys.stdout.write(text)


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = parser.add_subparsers(dest="command", required=True)
    scan_p = sub.add_parser("scan")
    scan_p.add_argument("--source", action="append", default=[], help="PROJECT/ID=KIND:PATH")
    scan_p.add_argument("--extensions", action="append", default=[], help="PROJECT=PATH, one source per extension directory")
    scan_p.add_argument("--out", required=True)
    scan_p.add_argument("--local-out")
    scan_p.add_argument("--jobs", type=int, default=1)
    scan_p.set_defaults(func=cmd_scan)
    cross = sub.add_parser("crosscheck-mdo")
    cross.add_argument("--aggregate", required=True)
    cross.add_argument("--mdo-root", required=True)
    cross.add_argument("--out")
    cross.set_defaults(func=cmd_crosscheck)
    args = parser.parse_args(argv)
    args.func(args)


if __name__ == "__main__":
    main()
