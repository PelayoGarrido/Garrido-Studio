import os
import shutil
import signal
import subprocess
import time

from desktop import ENV, ROOT


def run(args, reserve_gib=5, output=None, env=None):
    if shutil.disk_usage(ROOT).free < reserve_gib * 1024**3:
        raise SystemExit(f"La compilación cruzada conserva {reserve_gib} GiB antes de empezar")
    process = subprocess.Popen(args, cwd=ROOT, env=ENV if env is None else env, stdout=output, stderr=subprocess.STDOUT if output else None, start_new_session=True)
    start = time.monotonic()
    while process.poll() is None:
        if shutil.disk_usage(ROOT).free < 3 * 1024**3 or time.monotonic() - start > 1500:
            if os.name == "posix":
                os.killpg(process.pid, signal.SIGTERM)
            else:
                taskkill = os.path.join(os.environ.get("SystemRoot", r"C:\Windows"), "System32", "taskkill.exe")
                subprocess.run([taskkill, "/PID", str(process.pid), "/T", "/F"], capture_output=True, timeout=5, check=False)
            try:
                process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                if os.name == "posix":
                    os.killpg(process.pid, signal.SIGKILL)
                else:
                    process.kill()
                process.wait()
            raise SystemExit("Compilación detenida: reserva de espacio o tiempo agotada")
        time.sleep(1)
    if process.returncode:
        raise SystemExit(process.returncode)
