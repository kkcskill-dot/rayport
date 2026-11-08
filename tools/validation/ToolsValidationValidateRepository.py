"""Offline architecture checks. No service startup or third-party dependencies."""
from pathlib import Path
import json
import re
import sys
import unicodedata

ROOT = Path(__file__).resolve().parents[2]
EXCLUDED = {".git", "node_modules", "target", "dist", "__pycache__", "data", "imports", "workspaces"}
# Fixed tool discovery names: exact repository paths, never a basename wildcard.
FIXED_NAMES = {
    ".gitignore", "AGENTS.md", "README.md", "Cargo.toml", "Cargo.lock",
    "crates/rayport-server/Cargo.toml", "crates/rayport-domain/Cargo.toml",
    "crates/rayport-application/Cargo.toml", "crates/rayport-adapters/Cargo.toml",
    "apps/web/package.json", "apps/web/package-lock.json",
    "apps/web/tsconfig.json", "apps/web/index.html",
}
LAYERS = {"domain", "application", "infrastructure", "presentation", "bootstrap"}
ALLOWED = {
    "domain": {"domain"},
    "application": {"domain", "application"},
    "infrastructure": {"domain", "application", "infrastructure"},
    "presentation": {"domain", "application", "presentation"},
    "bootstrap": LAYERS,
}
IMPORT = re.compile(
    r"(?:\bfrom\s*|\bimport\s*\(\s*|\bimport\s*)(['\"])([^'\"]+)\1"
)


def pascal_path(parts):
    return "".join(word[:1].upper() + word[1:].lower()
                   for part in parts for word in re.split(r"[-_]", part) if word)


def layer(path):
    try:
        return path.relative_to(ROOT / "apps/web/src").parts[0]
    except ValueError:
        return None


def main():
    errors = []
    files = sorted(p for p in ROOT.rglob("*") if p.is_file()
                   and not any(part in EXCLUDED for part in p.relative_to(ROOT).parts)
                   and p.name != ".DS_Store")
    seen = {}
    import_count = 0
    for file in files:
        relative = file.relative_to(ROOT)
        label = relative.as_posix()
        if label not in FIXED_NAMES:
            prefix = pascal_path(relative.parts[:-1])
            if not file.stem.startswith(prefix):
                errors.append(f"{label}: expected path prefix {prefix}")
            suffix = file.stem[len(prefix):]
            if not re.fullmatch(r"[A-Z][A-Za-z0-9]*", suffix):
                errors.append(f"{label}: semantic filename must be PascalCase")
            if not re.fullmatch(r"\.[a-z0-9]+", file.suffix):
                errors.append(f"{label}: use one lowercase extension")
            for part in relative.parts[:-1]:
                if not re.fullmatch(r"[a-z][a-z0-9]*(?:-[a-z0-9]+)*", part):
                    errors.append(f"{label}: invalid directory segment {part}")
            key = unicodedata.normalize("NFC", file.name).casefold()
            if key in seen:
                errors.append(f"{label}: filename collides with {seen[key]}")
            seen[key] = label
        if file.suffix == ".json":
            try:
                json.loads(file.read_text())
            except (ValueError, UnicodeError) as error:
                errors.append(f"{label}: invalid JSON: {error}")
        if file.suffix in {".ts", ".tsx"}:
            content = file.read_text()
            origin = layer(file)
            for _, spec in IMPORT.findall(content):
                if not spec.startswith("."):
                    if origin == "domain":
                        errors.append(f"{label}: domain must not depend on package {spec}")
                    if origin == "application" and spec.split("/")[0] in {"react", "react-dom", "next", "vite"}:
                        errors.append(f"{label}: application must not depend on UI framework {spec}")
                    continue
                import_count += 1
                base = (file.parent / spec).resolve()
                candidates = [base] if base.suffix else [Path(str(base) + ext) for ext in (".ts", ".tsx", ".css")]
                target = next((p for p in candidates if p.is_file()), None)
                if target is None:
                    errors.append(f"{label}: unresolved import {spec}")
                    continue
                destination = layer(target)
                if origin in ALLOWED and destination not in ALLOWED[origin]:
                    errors.append(f"{label}: forbidden layer dependency {origin} -> {destination}")
        if file.suffix == ".md":
            for target in re.findall(r"\[[^\]]*\]\(([^)]+)\)", file.read_text()):
                if "://" in target or target.startswith("#"):
                    continue
                local = target.split("#")[0]
                if local and not (file.parent / local).exists():
                    errors.append(f"{label}: broken local Markdown link {target}")
    main_html = ROOT / "apps/web/index.html"
    if main_html.exists():
        for entry in re.findall(r'<script[^>]*src="([^"]+)"', main_html.read_text()):
            if not (ROOT / "apps/web" / entry.lstrip("/")).is_file():
                errors.append(f"apps/web/index.html: missing script entry {entry}")
    if errors:
        print("Repository validation FAILED:")
        print("\n".join("- " + error for error in errors))
        return 1
    print(f"PASS: {len(files)} files; path-based names, casefold uniqueness, JSON, "
          f"{import_count} relative imports, frontend layer boundaries, local document links.")
    print("Static import scan only; compiler/type checker and runtime checks remain separate.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
