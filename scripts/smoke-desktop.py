"""Verify that a packaged GUI stays alive under the CI display server."""

import os
from pathlib import Path
import signal
import subprocess
import sys
import time

with subprocess.Popen(
    [str(Path(sys.argv[1]).resolve())], start_new_session=True
) as process:
    try:
        time.sleep(10)
        if process.poll() is not None:
            raise SystemExit(f"Desktop app exited during startup: {process.returncode}")
        print("Packaged desktop app stayed running for 10 seconds.")
    finally:
        if process.poll() is None:
            os.killpg(process.pid, signal.SIGTERM)
            try:
                process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                os.killpg(process.pid, signal.SIGKILL)
                process.wait()
