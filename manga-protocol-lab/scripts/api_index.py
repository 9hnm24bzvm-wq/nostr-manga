"""Generate/check the wrapper's declared HTTP route inventory (not OpenAPI)."""

from pathlib import Path
import argparse
import re
from urllib.parse import unquote, urlsplit

ROOT = Path(__file__).resolve().parents[1]
START = "<!-- generated-routes:start -->"
END = "<!-- generated-routes:end -->"


def routes(source):
    rows = []
    for match in re.finditer(r'\.route\(\s*"([^"\\]+)"\s*,', source):
        depth, pos, quoted, escaped = 1, match.end(), False, False
        begin = pos
        while pos < len(source) and depth:
            ch = source[pos]
            if quoted:
                if escaped:
                    escaped = False
                elif ch == "\\":
                    escaped = True
                elif ch == '"':
                    quoted = False
            elif ch == '"':
                quoted = True
            elif ch == "(":
                depth += 1
            elif ch == ")":
                depth -= 1
            pos += 1
        if depth:
            raise ValueError("Unclosed route: " + match[1])
        methods = re.findall(
            r"\b(get|post|put|delete|patch|head|options)\(\s*([\w:]+)\s*\)",
            source[begin : pos - 1],
        )
        if not methods:
            raise ValueError("Unsupported route declaration: " + match[1])
        rows.extend((match[1], method.upper(), handler) for method, handler in methods)
    if len(list(re.finditer(r"\.route\s*\(", source))) != len(
        set(path for path, _, _ in rows)
    ):
        raise ValueError(
            "Dynamic or duplicate route declaration requires explicit review"
        )
    if len({(p, m) for p, m, _ in rows}) != len(rows):
        raise ValueError("Duplicate path/method")
    return sorted(rows)


def render(rows):
    lines = [
        START,
        "",
        f"{len(set(p for p, _, _ in rows))} declared paths; {len(rows)} explicit path/method pairs.",
        "",
        "| Method | Path | Implementation entry point |",
        "|---|---|---|",
    ]
    lines += [
        f"| {method} | `{path}` | `{handler}` |" for path, method, handler in rows
    ]
    return "\n".join(lines) + "\n\n" + END


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    path = ROOT / "docs/API-INDEX.md"
    text = path.read_text()
    start, end = text.index(START), text.index(END) + len(END)
    actual = render(routes((ROOT / "server/src/node.rs").read_text()))
    result = text[:start] + actual + text[end:]
    if args.check:
        if text != result:
            raise SystemExit("API inventory is stale; run python3 scripts/api_index.py")
        for link in re.findall(r"\]\(([^)]+)\)", text):
            target = urlsplit(link)
            if target.scheme or target.netloc or not target.path:
                continue
            if not (path.parent / unquote(target.path)).is_file():
                raise SystemExit("Missing local API contract: " + link)
        print("API inventory matches every declared wrapper route")
    else:
        path.write_text(result)


if __name__ == "__main__":
    main()
