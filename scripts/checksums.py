"""Write release checksums without platform-specific shell utilities."""

import hashlib
from pathlib import Path
import sys

directory = Path(sys.argv[1])
output = directory / sys.argv[2]
with output.open("w", encoding="utf-8", newline="\n") as manifest:
    for path in sorted(directory.iterdir()):
        if path.is_file() and not path.name.endswith("-SHA256SUMS.txt"):
            with path.open("rb") as source:
                digest = hashlib.file_digest(source, "sha256").hexdigest()
            manifest.write(f"{digest}  {path.name}\n")
