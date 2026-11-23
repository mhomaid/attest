"""Download one registered dataset into the gitignored cache and verify its checksum."""

from __future__ import annotations

import argparse
import hashlib
import re
import sys
import urllib.request
from pathlib import Path
from urllib.parse import urlparse

if __package__:
    from .registry import index_registry, load_registry
else:
    from registry import index_registry, load_registry

_HEX = re.compile(r"^[0-9a-f]+$")
_LENGTH = {"md5": 32, "sha256": 64}


class ChecksumMismatch(Exception):
    """The downloaded bytes do not match the registry digest, or none is pinned."""


def parse_checksum(value: object) -> tuple[str, str | None]:
    if not isinstance(value, str) or ":" not in value:
        raise ChecksumMismatch(
            "checksum must look like 'sha256:<hex>' or 'md5:<hex>', "
            f"got {value!r}"
        )
    algo, digest = value.split(":", 1)
    algo = algo.lower()
    digest = digest.lower()
    if algo not in _LENGTH:
        raise ChecksumMismatch(f"unsupported checksum algorithm {algo}")
    if not _HEX.fullmatch(digest) or len(digest) != _LENGTH[algo]:
        return algo, None
    return algo, digest


def _hash_file(path: Path, algo: str) -> str:
    digest = hashlib.new(algo)
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def _download(url: str, dest: Path) -> None:
    request = urllib.request.Request(url, headers={"User-Agent": "attest-dataset-fetch"})
    with urllib.request.urlopen(request, timeout=120) as response, dest.open("wb") as out:
        while True:
            chunk = response.read(1024 * 1024)
            if not chunk:
                break
            out.write(chunk)


def _file_specs(entry: dict) -> list[dict]:
    listed = entry.get("files")
    if isinstance(listed, list) and listed:
        specs = []
        for item in listed:
            if not isinstance(item, dict):
                raise ChecksumMismatch("each files entry must be a mapping")
            specs.append(item)
        return specs
    return [
        {
            "source_url": entry.get("source_url"),
            "filename": entry.get("filename"),
            "checksum": entry.get("checksum"),
        }
    ]


def _store_one(dataset_id: str, spec: dict, dest_dir: Path) -> Path:
    algo, expected = parse_checksum(spec.get("checksum"))
    if expected is None:
        raise ChecksumMismatch(
            f"{dataset_id}: checksum {spec.get('checksum')!r} is not a pinned "
            "sha256 or md5 digest; refusing to store unverified bytes"
        )
    url = str(spec.get("source_url") or "")
    if not url:
        raise ChecksumMismatch(f"{dataset_id}: file has no source_url")
    filename = str(spec.get("filename") or Path(urlparse(url).path).name or f"{dataset_id}.bin")
    if not filename or filename in {".", "/"}:
        filename = f"{dataset_id}.bin"
    dest = dest_dir / filename
    partial = dest_dir / f"{filename}.partial"
    try:
        _download(url, partial)
        actual = _hash_file(partial, algo)
        if actual != expected:
            raise ChecksumMismatch(
                f"{dataset_id}/{filename}: expected {algo}:{expected}, got {algo}:{actual}"
            )
        partial.replace(dest)
    finally:
        partial.unlink(missing_ok=True)
    return dest


def fetch_dataset(dataset_id: str, registry_path: Path, cache_dir: Path) -> Path:
    by_id = index_registry(load_registry(registry_path))
    entry = by_id.get(dataset_id)
    if entry is None:
        raise ChecksumMismatch(f"unknown dataset {dataset_id}")
    dest_dir = Path(cache_dir) / dataset_id
    dest_dir.mkdir(parents=True, exist_ok=True)
    stored: list[Path] = []
    for spec in _file_specs(entry):
        stored.append(_store_one(dataset_id, spec, dest_dir))
    return stored[0]


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Fetch one registered dataset and verify its checksum")
    parser.add_argument("--id", required=True)
    parser.add_argument("--registry", type=Path, required=True)
    parser.add_argument("--cache", type=Path, required=True)
    args = parser.parse_args(argv)
    try:
        stored = fetch_dataset(args.id, registry_path=args.registry, cache_dir=args.cache)
    except (ChecksumMismatch, OSError, ValueError) as exc:
        print(f"error: {exc}", file=sys.stderr)
        return 1
    print(stored)
    return 0


if __name__ == "__main__":
    sys.exit(main())
