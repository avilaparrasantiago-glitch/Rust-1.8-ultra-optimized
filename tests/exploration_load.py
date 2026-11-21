"""Run with CORAL_BIN pointing to the build under measurement."""
import ctypes
import json
import os
from pathlib import Path
import tempfile
import time
import urllib.request

from protocol47 import Client, server


def rss(proc):
    if os.name == "nt":
        class Counters(ctypes.Structure):
            _fields_ = [("size", ctypes.c_uint32), ("faults", ctypes.c_uint32)] + [
                (name, ctypes.c_size_t) for name in
                ("peak", "working", "peak_paged", "paged", "peak_nonpaged", "nonpaged", "pagefile", "peak_pagefile")
            ]
        value = Counters()
        value.size = ctypes.sizeof(value)
        if not ctypes.windll.psapi.GetProcessMemoryInfo(ctypes.c_void_p(int(proc._handle)),
                                                       ctypes.byref(value), value.size):
            raise ctypes.WinError()
        return value.working
    status = Path(f"/proc/{proc.pid}/status").read_text()
    return int(next(line for line in status.splitlines() if line.startswith("VmRSS:")).split()[1]) * 1024


def metrics(port):
    with urllib.request.urlopen(f"http://127.0.0.1:{port}/metrics", timeout=2) as response:
        return {key: int(value) for key, value in
                (line.split() for line in response.read().decode().splitlines())}


def run():
    with tempfile.TemporaryDirectory() as temp:
        Path(temp, "seed").write_text("9581")
        procs = []
        with server(started=procs.append, arena_file=None, terrain="normal", worlds=["lobby","hell","end"],
                    dimensions={"hell":"nether","end":"end"},
                    view_distance=1, data_dir=temp) as (port, monitoring):
            proc = procs[0]
            idle = rss(proc)
            clients = [Client(port, 2), Client(port, 2)]
            try:
                first_column_ms = []
                for client, name in zip(clients, ("Alice", "Bob")):
                    login_start = time.perf_counter_ns()
                    client.login(name)
                    client.confirm_pos()
                    client.until(0x26)
                    first_column_ms.append((time.perf_counter_ns() - login_start) / 1e6)
                initial_bytes = [client.received_bytes for client in clients]
                before = metrics(monitoring)
                start = time.monotonic()
                peak = rss(proc)
                for step in range(14):
                    if step in (4, 8, 12):
                        destination = {4:"hell",8:"end",12:"lobby"}[step]
                        for client in clients:
                            client.chat("/world tp " + destination)
                            client.confirm_pos()
                            client.until(0x26)
                    for index, (client, name) in enumerate(zip(clients, ("Alice", "Bob"))):
                        # Alice operates both teleports. The clients independently verify receipt.
                        cx = (step + 2) * (6 if index == 0 else -6)
                        clients[0].chat(f"/tp {name} {cx * 16 + .5} 100 160.5")
                        client.confirm_pos()
                        client.until(0x21, lambda b: int.from_bytes(b[:4], "big", signed=True) == cx
                                     and int.from_bytes(b[4:8], "big", signed=True) == 10
                                     and b[9:11] != bytes(2))
                    time.sleep(1.95)
                    peak = max(peak, rss(proc))
                after = metrics(monitoring)
                seconds = time.monotonic() - start
                diff = {k: after[k] - v for k, v in before.items()}
                ticks = diff["coral_tick_seconds_count"]
                bounds = [k.split('"')[1] for k,v in diff.items()
                          if "_bucket" in k and v >= .99 * ticks]
                columns = diff["coral_generated_columns_total"]
                print(json.dumps(dict(seconds=seconds, ticks=ticks, worlds=3, mean_tps_per_world=ticks / seconds / 3,
                                      p99_upper_seconds=bounds[0], overruns=diff["coral_tick_overruns_total"],
                                      generated_columns=columns,
                                      cold_mean_us=diff["coral_generation_nanoseconds_total"]/columns/1000,
                                      login_to_first_bulk_ms=first_column_ms,
                                      exploration_bytes_read_per_player=[
                                          c.received_bytes - n for c, n in zip(clients, initial_bytes)
                                      ],
                                      idle_RSS=idle, peak_sampled_RSS=peak, players=2), indent=2))
            finally:
                for client in clients:
                    client.s.close()


if __name__ == "__main__":
    run()
