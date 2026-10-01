#!/usr/bin/env python3
"""Automated experiment runner for the measurement campaign (thesis Ch. 6).

For each repetition of a scenario it:
  1. applies the platform's initial-conditions protocol (service reset /
     homing motion), so every run starts from an equivalent state;
  2. starts `ros2 bag record` for the platform's topic list (markers +
     commands + feedback share one clock domain — see metrics.rs);
  3. submits the program through the platform's public REST API
     (`POST /api/execute` is synchronous: it returns when the program
     finishes), optionally firing `POST /api/abort` after a fixed delay
     for E-STOP scenarios;
  4. stops the bag and archives it with a metadata.json.

Usage:
  python3 runner.py <scenario> --platform turtlesim|waffle|arm [-n 30]
  python3 runner.py --list
  python3 runner.py --rtt 100          # sample HTTP RTT (browser-hop estimate)

The corresponding simulator (turtlesim / turtlebot3_gazebo / twobot) and
the Rust backend must be running.
"""

import argparse
import json
import shutil
import signal
import statistics
import subprocess
import sys
import threading
import time
from pathlib import Path

import requests

from scenarios import MARKER_TOPIC, PLATFORMS, get_scenarios

API = "http://localhost:3000"
BAG_WARMUP_S = 6.0   # rosbag subscription discovery before the program starts
                     # (6 s: Gazebo's heavier startup needs more than the 2 s
                     #  that sufficed for Turtlesim; see integrity diagnostic)
BAG_DRAIN_S = 1.0    # capture trailing messages after the program ends
MAX_RETRIES = 3      # redo a run if its bag missed the exec_request marker


def bag_has_exec_request(bag_dir: Path) -> bool:
    """True if the recorded bag captured the exec_request marker, i.e. the
    recorder was subscribed before the run started. Runs that fail this
    are re-executed (the bag missed the beginning)."""
    try:
        import rosbag2_py
        from rclpy.serialization import deserialize_message
        from std_msgs.msg import String

        reader = rosbag2_py.SequentialReader()
        reader.open(rosbag2_py.StorageOptions(uri=str(bag_dir)),
                    rosbag2_py.ConverterOptions("", ""))
        while reader.has_next():
            topic, raw, _ = reader.read_next()
            if topic == MARKER_TOPIC:
                data = json.loads(deserialize_message(raw, String).data)
                if data.get("event") == "exec_request":
                    return True
        return False
    except Exception:  # noqa: BLE001
        return False


class ResourceSampler:
    """Backend engineering footprint during one run.

    CPU: true utilization over the run window, from /proc/<pid>/stat
    utime+stime deltas (`ps -o %cpu` reports a process-lifetime average,
    useless for short runs). RSS: sampled every 0.5 s.
    """

    def __init__(self) -> None:
        try:
            out = subprocess.run(
                ["pgrep", "-f", "target/debug/rust_app"],
                capture_output=True, text=True, check=True,
            )
            self.pid = out.stdout.split()[0]
        except Exception:  # noqa: BLE001
            self.pid = None
        self.rss_samples: list[float] = []
        self._stop = threading.Event()
        self._thread: threading.Thread | None = None
        self._ticks0 = 0
        self._t0 = 0.0
        self._cpu_percent: float | None = None

    def _cpu_ticks(self) -> int:
        with open(f"/proc/{self.pid}/stat") as f:
            parts = f.read().split()
        return int(parts[13]) + int(parts[14])  # utime + stime

    def _loop(self) -> None:
        while not self._stop.is_set():
            try:
                with open(f"/proc/{self.pid}/status") as f:
                    for line in f:
                        if line.startswith("VmRSS:"):
                            self.rss_samples.append(int(line.split()[1]) / 1024.0)  # MB
                            break
            except Exception:  # noqa: BLE001
                pass
            self._stop.wait(0.5)

    def __enter__(self) -> "ResourceSampler":
        if self.pid:
            try:
                self._ticks0 = self._cpu_ticks()
                self._t0 = time.monotonic()
                self._thread = threading.Thread(target=self._loop, daemon=True)
                self._thread.start()
            except Exception:  # noqa: BLE001
                self.pid = None
        return self

    def __exit__(self, *_: object) -> None:
        self._stop.set()
        if self._thread:
            self._thread.join(timeout=2)
        if self.pid:
            try:
                wall = time.monotonic() - self._t0
                dt_ticks = self._cpu_ticks() - self._ticks0
                import os
                self._cpu_percent = (dt_ticks / os.sysconf("SC_CLK_TCK")) / wall * 100
            except Exception:  # noqa: BLE001
                pass

    def summary(self) -> dict | None:
        if self._cpu_percent is None and not self.rss_samples:
            return None
        out: dict = {}
        if self._cpu_percent is not None:
            out["cpu_percent_run"] = round(self._cpu_percent, 2)
        if self.rss_samples:
            out["rss_mb_mean"] = round(statistics.mean(self.rss_samples), 1)
            out["rss_mb_max"] = round(max(self.rss_samples), 1)
        return out


def check_backend() -> None:
    try:
        requests.get(f"{API}/api/profiles", timeout=3).raise_for_status()
    except Exception as exc:  # noqa: BLE001
        sys.exit(f"Backend not reachable at {API}: {exc}")


def select_profile(profile_id: str) -> None:
    r = requests.post(f"{API}/api/profiles/select", json={"id": profile_id}, timeout=10)
    r.raise_for_status()
    print(f"  profile: {r.json()['message']}")


def apply_reset(platform_cfg: dict) -> None:
    """Initial-conditions protocol: service reset and/or homing motion."""
    if cmd := platform_cfg.get("reset_cmd"):
        subprocess.run(cmd, check=True, capture_output=True, timeout=20)
    if home := platform_cfg.get("home_program"):
        r = requests.post(f"{API}/api/execute", json=home, timeout=60)
        r.raise_for_status()
    time.sleep(0.5)  # let the state settle


def start_bag(out_dir: Path, topics: list[str]) -> subprocess.Popen:
    proc = subprocess.Popen(
        ["ros2", "bag", "record", "-o", str(out_dir), *topics],
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )
    time.sleep(BAG_WARMUP_S)
    return proc


def stop_bag(proc: subprocess.Popen) -> None:
    time.sleep(BAG_DRAIN_S)
    proc.send_signal(signal.SIGINT)
    try:
        proc.wait(timeout=15)
    except subprocess.TimeoutExpired:
        proc.kill()


def run_once(run_dir: Path, platform_cfg: dict, scenario: dict, scenario_name: str, run_idx: int) -> dict:
    apply_reset(platform_cfg)

    bag = start_bag(run_dir / "bag", platform_cfg["topics"])
    program = {"name": f"{scenario_name}_run{run_idx:03d}", "steps": scenario["steps"]}

    abort_result: dict = {}
    abort_thread = None
    if delay := scenario.get("abort_after_s"):
        def fire_abort() -> None:
            time.sleep(delay)
            t0 = time.monotonic()
            r = requests.post(f"{API}/api/abort", timeout=10)
            abort_result.update(
                {"http_s": time.monotonic() - t0, "response": r.json(), "fired_after_s": delay}
            )

        abort_thread = threading.Thread(target=fire_abort)
        abort_thread.start()

    with ResourceSampler() as sampler:
        t0 = time.monotonic()
        resp = requests.post(f"{API}/api/execute", json=program, timeout=600)
        wall_s = time.monotonic() - t0
    if abort_thread:
        abort_thread.join()

    stop_bag(bag)

    meta = {
        "scenario": scenario_name,
        "run": run_idx,
        "steps": len(scenario["steps"]),
        # Full program: makes each run self-contained for the analysis
        # pipeline (ideal geometry is derived from these steps)
        "program": program,
        "execute_http_status": resp.status_code,
        "execute_response": resp.json(),
        "execute_wall_s": round(wall_s, 4),
        "abort": abort_result or None,
        "backend_resources": sampler.summary(),
        "timestamp": time.strftime("%Y-%m-%dT%H:%M:%S"),
    }
    (run_dir / "metadata.json").write_text(json.dumps(meta, indent=2))
    return meta


def sample_rtt(samples: int) -> None:
    """HTTP round-trip of a lightweight endpoint: the client-side estimate
    of the browser→backend hop (reported separately from the DDS-domain
    latency, since the two clocks cannot be compared directly)."""
    times = []
    for _ in range(samples):
        t0 = time.monotonic()
        requests.get(f"{API}/api/profiles/active", timeout=5).raise_for_status()
        times.append((time.monotonic() - t0) * 1000)
        time.sleep(0.05)
    out = Path("data/http_rtt_ms.json")
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text(json.dumps({"samples_ms": times}, indent=2))
    import statistics

    print(f"HTTP RTT over {samples} samples: mean={statistics.mean(times):.2f} ms, "
          f"stdev={statistics.stdev(times):.2f} ms → saved to {out}")


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("scenario", nargs="?", help="scenario name (see --list)")
    ap.add_argument("--platform", choices=PLATFORMS.keys())
    ap.add_argument("-n", "--runs", type=int, default=30)
    ap.add_argument("--out", default="data")
    ap.add_argument("--list", action="store_true")
    ap.add_argument("--rtt", type=int, metavar="N", help="sample HTTP RTT N times and exit")
    args = ap.parse_args()

    if args.list:
        for platform in PLATFORMS:
            print(f"{platform}: {', '.join(get_scenarios(platform))}")
        return

    check_backend()

    if args.rtt:
        sample_rtt(args.rtt)
        return

    if not args.scenario or not args.platform:
        ap.error("scenario and --platform are required (or use --list / --rtt)")

    scenarios = get_scenarios(args.platform)
    if args.scenario not in scenarios:
        ap.error(f"unknown scenario {args.scenario!r} for {args.platform} "
                 f"(available: {', '.join(scenarios)})")

    platform_cfg = PLATFORMS[args.platform]
    scenario = scenarios[args.scenario]
    base = Path(args.out) / args.platform / args.scenario

    print(f"Campaign: {args.scenario} on {args.platform}, n={args.runs} → {base}/")
    select_profile(platform_cfg["profile_id"])

    for i in range(1, args.runs + 1):
        run_dir = base / f"run_{i:03d}"
        # Skip only runs already recorded WITH a valid exec_request marker;
        # runs whose bag missed the beginning are re-executed.
        if (run_dir / "metadata.json").exists() and bag_has_exec_request(run_dir / "bag"):
            print(f"  run {i:03d}: exists and complete, skipping")
            continue

        for attempt in range(1, MAX_RETRIES + 1):
            if run_dir.exists():
                shutil.rmtree(run_dir)
            run_dir.mkdir(parents=True, exist_ok=True)
            meta = run_once(run_dir, platform_cfg, scenario, args.scenario, i)
            status = meta["execute_response"].get("status", "?")
            if bag_has_exec_request(run_dir / "bag"):
                print(f"  run {i:03d}: {status} in {meta['execute_wall_s']}s")
                break
            print(f"  run {i:03d}: bag missed exec_request, retry {attempt}/{MAX_RETRIES}")
        else:
            print(f"  run {i:03d}: FAILED to capture exec_request after {MAX_RETRIES} attempts")

    print("Done.")


if __name__ == "__main__":
    main()
