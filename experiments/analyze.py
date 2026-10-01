#!/usr/bin/env python3
"""Analysis pipeline for the measurement campaign (thesis Ch. 6).

Reads the bags recorded by runner.py and computes, per run:

  - Systemic latency L_s = t(first velocity command) - t(exec_request
    marker), decomposed into engine dispatch, block startup and
    simulator motion onset — all timestamps from ONE bag clock.
    NOTE: L_s (total) compares two DIRECTLY-published messages and is
    the primary metric. The decomposition uses the STEP_START marker,
    which crosses the event-bus bridge (~0.5 ms extra), so lat_block can
    come out slightly negative; components are indicative and their
    marker delay equals what the UI feedback path itself experiences.
  - E-STOP determinism = t(first zero-velocity command / hold
    trajectory) - t(abort_request marker).
  - Positional error E_p: per-block displacement/rotation vs the
    commanded geometry (segmented by step markers) and final pose error
    vs the ideal trajectory derived from the program itself. For the
    arm: per-joint articular error at each step's end.
  - Event delivery completeness: structural markers present vs expected.

Aggregates mean/std/min/max across runs (summary.csv + aggregate.json)
and renders one representative figure set per scenario (run_001) plus
cross-run aggregate figures (latency histogram, E-STOP boxplot, and the
all-runs trajectory overlay that evidences repeatability).

Usage (ROS environment must be sourced for rosbag2_py):
  .venv/bin/python analyze.py data/ --results results/
"""

import argparse
import json
import math
from pathlib import Path

import matplotlib
matplotlib.use("Agg")
import matplotlib.pyplot as plt
import pandas as pd

import rosbag2_py
from rclpy.serialization import deserialize_message
from geometry_msgs.msg import Twist, TwistStamped
from nav_msgs.msg import Odometry
from sensor_msgs.msg import JointState
from std_msgs.msg import String
from trajectory_msgs.msg import JointTrajectory
from turtlesim.msg import Pose as TurtlePose

MOTION_EPS_M = 0.005      # displacement that counts as "motion started"
MOTION_EPS_RAD = 0.01     # joint delta that counts as "motion started"

# ─── Bag loading ─────────────────────────────────────────────────────────────

MSG_TYPES = {
    "std_msgs/msg/String": String,
    "geometry_msgs/msg/Twist": Twist,
    "geometry_msgs/msg/TwistStamped": TwistStamped,
    "turtlesim/msg/Pose": TurtlePose,
    "nav_msgs/msg/Odometry": Odometry,
    "sensor_msgs/msg/JointState": JointState,
    "trajectory_msgs/msg/JointTrajectory": JointTrajectory,
}


def wrap_angle(a: float) -> float:
    return (a + math.pi) % (2 * math.pi) - math.pi


def yaw_from_quat(q) -> float:
    siny = 2.0 * (q.w * q.z + q.x * q.y)
    cosy = 1.0 - 2.0 * (q.y * q.y + q.z * q.z)
    return math.atan2(siny, cosy)


class BagData:
    """All campaign-relevant messages of one run, timestamps in seconds."""

    def __init__(self, bag_dir: Path):
        self.markers: list[tuple[float, str, dict]] = []   # (t, event, payload)
        self.cmd_vel: list[tuple[float, float, float]] = []  # (t, lin, ang)
        self.pose: list[tuple[float, float, float, float]] = []  # (t, x, y, yaw)
        self.joints: list[tuple[float, list[float]]] = []  # (t, positions by name order)
        self.trajectories: list[tuple[float, int]] = []    # (t, n_points)
        self._joint_order: list[str] | None = None

        reader = rosbag2_py.SequentialReader()
        reader.open(rosbag2_py.StorageOptions(uri=str(bag_dir)),
                    rosbag2_py.ConverterOptions("", ""))
        types = {t.name: t.type for t in reader.get_all_topics_and_types()}

        while reader.has_next():
            topic, raw, t_ns = reader.read_next()
            cls = MSG_TYPES.get(types[topic])
            if cls is None:
                continue
            t = t_ns / 1e9
            msg = deserialize_message(raw, cls)

            if cls is String:
                data = json.loads(msg.data)
                event = data["event"]
                payload = data.get("payload", {})
                if event == "system_event":
                    event = payload.get("event_type", "system_event")
                self.markers.append((t, event, payload))
            elif cls is Twist:
                self.cmd_vel.append((t, msg.linear.x, msg.angular.z))
            elif cls is TwistStamped:
                self.cmd_vel.append((t, msg.twist.linear.x, msg.twist.angular.z))
            elif cls is TurtlePose:
                self.pose.append((t, msg.x, msg.y, wrap_angle(msg.theta)))
            elif cls is Odometry:
                p = msg.pose.pose
                self.pose.append((t, p.position.x, p.position.y, yaw_from_quat(p.orientation)))
            elif cls is JointState:
                if self._joint_order is None:
                    self._joint_order = list(msg.name)
                by_name = dict(zip(msg.name, msg.position))
                self.joints.append((t, [by_name.get(n, math.nan) for n in self._joint_order]))
            elif cls is JointTrajectory:
                self.trajectories.append((t, len(msg.points)))

    # ── helpers ──────────────────────────────────────────────────────

    def marker_times(self, event: str) -> list[float]:
        return [t for t, e, _ in self.markers if e == event]

    def first_marker(self, event: str) -> float | None:
        times = self.marker_times(event)
        return times[0] if times else None

    def pose_at(self, t: float):
        """Last pose sample at or before t (None if none)."""
        prev = None
        for sample in self.pose:
            if sample[0] > t:
                break
            prev = sample
        return prev

    def joints_at(self, t: float):
        prev = None
        for sample in self.joints:
            if sample[0] > t:
                break
            prev = sample
        return prev


# ─── Ideal geometry from the program itself ──────────────────────────────────


def ideal_final_pose(steps: list[dict]) -> tuple[float, float, float]:
    """Dead-reckons the commanded geometry: relative ideal (x, y, yaw)."""
    x = y = yaw = 0.0
    for s in steps:
        if s["type"] == "move_odometry":
            d = s["target_distance"]
            x += d * math.cos(yaw)
            y += d * math.sin(yaw)
        elif s["type"] == "rotate":
            yaw = wrap_angle(yaw + math.radians(s["angle_degrees"]))
    return x, y, yaw


def ideal_polyline(steps: list[dict]) -> list[tuple[float, float]]:
    pts = [(0.0, 0.0)]
    x = y = yaw = 0.0
    for s in steps:
        if s["type"] == "move_odometry":
            d = s["target_distance"]
            x += d * math.cos(yaw)
            y += d * math.sin(yaw)
            pts.append((x, y))
        elif s["type"] == "rotate":
            yaw = wrap_angle(yaw + math.radians(s["angle_degrees"]))
    return pts


# ─── Per-run metric extraction ───────────────────────────────────────────────


def analyze_run(run_dir: Path) -> dict | None:
    meta = json.loads((run_dir / "metadata.json").read_text())
    bag = BagData(run_dir / "bag")
    steps = meta["program"]["steps"]
    is_arm = any(s["type"] == "joint_move" for s in steps)
    row: dict = {"run": meta["run"], "steps": meta["steps"],
                 "wall_s": meta["execute_wall_s"],
                 "status": meta["execute_response"].get("status")}
    if res := meta.get("backend_resources"):
        row["cpu_run_pct"] = res.get("cpu_percent_run")
        row["rss_mean_mb"] = res.get("rss_mb_mean")

    t_req = bag.first_marker("exec_request")
    if t_req is None:
        return None

    # ── Latency decomposition (all from the bag clock) ───────────────
    t_step = next((t for t in bag.marker_times("STEP_START") if t >= t_req), None)
    if is_arm:
        t_cmd = next((t for t, _ in bag.trajectories if t >= t_req), None)
    else:
        t_cmd = next((t for t, *_ in bag.cmd_vel if t >= t_req), None)
    row["lat_total_ms"] = (t_cmd - t_req) * 1000 if t_cmd else None
    row["lat_engine_ms"] = (t_step - t_req) * 1000 if t_step else None
    row["lat_block_ms"] = (t_cmd - t_step) * 1000 if (t_cmd and t_step) else None

    # Motion onset: simulator/robot response after the first command
    if t_cmd:
        if is_arm and bag.joints:
            q0 = bag.joints_at(t_cmd)
            onset = next((t for t, q in bag.joints if t > t_cmd and q0 and
                          max(abs(a - b) for a, b in zip(q, q0[1])) > MOTION_EPS_RAD), None)
        elif bag.pose:
            p0 = bag.pose_at(t_cmd) or bag.pose[0]
            onset = next((t for t, x, y, _ in bag.pose if t > t_cmd and
                          math.hypot(x - p0[1], y - p0[2]) > MOTION_EPS_M), None)
        else:
            onset = None
        row["lat_motion_onset_ms"] = (onset - t_cmd) * 1000 if onset else None

    # ── E-STOP determinism ───────────────────────────────────────────
    t_abort = bag.first_marker("abort_request")
    if t_abort is not None:
        if is_arm:
            t_stop = next((t for t, _ in bag.trajectories if t > t_abort), None)
        else:
            t_stop = next((t for t, lin, ang in bag.cmd_vel
                           if t > t_abort and lin == 0.0 and ang == 0.0), None)
        row["estop_cmd_ms"] = (t_stop - t_abort) * 1000 if t_stop else None
        t_ev = next((t for t in bag.marker_times("PROGRAM_ABORT") if t > t_abort), None)
        row["estop_event_ms"] = (t_ev - t_abort) * 1000 if t_ev else None

    # ── Positional error E_p (completed runs only) ───────────────────
    t_finish = bag.first_marker("PROGRAM_FINISH")
    if t_finish and t_abort is None:
        if is_arm and bag.joints:
            # Articular error at the end of each joint_move step: STEP_FINISH
            # markers cover ALL steps in order, so index them by step position
            finishes = bag.marker_times("STEP_FINISH")
            errors = []
            for idx, s in enumerate(steps):
                if s["type"] != "joint_move" or idx >= len(finishes):
                    continue
                sample = bag.joints_at(finishes[idx] + 0.05)
                if sample:
                    errors.append(max(abs(a - b) for a, b
                                      in zip(sample[1], s["positions"])))
            if errors:
                row["ep_joint_max_rad"] = max(errors)
                row["ep_joint_mean_rad"] = sum(errors) / len(errors)
        elif bag.pose:
            # If bag discovery missed odometry samples before the request,
            # the first sample is a valid start reference: the robot has
            # not moved ~1 ms after the request arrives.
            p_start = bag.pose_at(t_req) or bag.pose[0]
            p_end = bag.pose_at(t_finish + 0.1) or bag.pose[-1]
            ix, iy, iyaw = ideal_final_pose(steps)
            # Real displacement expressed in the run's starting frame
            dx_w, dy_w = p_end[1] - p_start[1], p_end[2] - p_start[2]
            c, s0 = math.cos(-p_start[3]), math.sin(-p_start[3])
            rx, ry = dx_w * c - dy_w * s0, dx_w * s0 + dy_w * c
            ryaw = wrap_angle(p_end[3] - p_start[3])
            row["ep_final_m"] = math.hypot(rx - ix, ry - iy)
            row["ep_yaw_deg"] = abs(math.degrees(wrap_angle(ryaw - iyaw)))
            row["final_x_m"], row["final_y_m"] = rx, ry

    # ── Delivery completeness ────────────────────────────────────────
    n_steps = len(steps)
    expected_ok = (len(bag.marker_times("STEP_START")) >= (1 if t_abort else n_steps)
                   and (t_abort is not None or t_finish is not None))
    row["markers_complete"] = expected_ok

    return row


# ─── Figures (one representative run + aggregates, per user request) ─────────


def fig_trajectory(bag: BagData, steps: list[dict], out: Path, title: str) -> None:
    if not bag.pose:
        return
    t_req = bag.first_marker("exec_request") or bag.pose[0][0]
    p0 = bag.pose_at(t_req) or bag.pose[0]
    c, s0 = math.cos(-p0[3]), math.sin(-p0[3])

    def rel(x: float, y: float) -> tuple[float, float]:
        dx, dy = x - p0[1], y - p0[2]
        return dx * c - dy * s0, dx * s0 + dy * c

    real = [rel(x, y) for _, x, y, _ in bag.pose]
    ideal = ideal_polyline(steps)
    fig, ax = plt.subplots(figsize=(6, 6))
    ax.plot([p[0] for p in ideal], [p[1] for p in ideal], "k--", lw=1.5,
            marker="o", ms=4, label="Trayectoria ideal")
    ax.plot([p[0] for p in real], [p[1] for p in real], color="tab:blue",
            lw=1.2, label="Trayectoria real (odometría)")
    ax.set_xlabel("x [m]"); ax.set_ylabel("y [m]")
    ax.set_title(title); ax.legend(); ax.axis("equal"); ax.grid(alpha=0.3)
    fig.savefig(out, dpi=150, bbox_inches="tight"); plt.close(fig)


def fig_timeseries(bag: BagData, out: Path, title: str) -> None:
    if not bag.pose:
        return
    t0 = bag.pose[0][0]
    ts = [t - t0 for t, *_ in bag.pose]
    fig, axes = plt.subplots(3, 1, figsize=(8, 6), sharex=True)
    for ax, idx, lab in ((axes[0], 1, "x [m]"), (axes[1], 2, "y [m]"), (axes[2], 3, "yaw [rad]")):
        ax.plot(ts, [p[idx] for p in bag.pose], lw=1.0)
        ax.set_ylabel(lab); ax.grid(alpha=0.3)
    for t in bag.marker_times("STEP_START"):
        for ax in axes:
            ax.axvline(t - t0, color="tab:green", alpha=0.35, lw=0.8)
    axes[2].set_xlabel("t [s]")
    axes[0].set_title(f"{title} — posición vs tiempo (líneas verdes: inicio de bloque)")
    fig.savefig(out, dpi=150, bbox_inches="tight"); plt.close(fig)


def fig_joints(bag: BagData, steps: list[dict], out: Path, title: str) -> None:
    if not bag.joints:
        return
    t0 = bag.joints[0][0]
    ts = [t - t0 for t, _ in bag.joints]
    n = len(bag.joints[0][1])
    fig, ax = plt.subplots(figsize=(8, 4.5))
    labels = ["Codo (flex/ext)", "Muñeca (pron/sup)"] + [f"q{i}" for i in range(2, n)]
    for i in range(n):
        ax.plot(ts, [q[i] for _, q in bag.joints], lw=1.2, label=labels[i])
    for s in steps:
        if s["type"] == "joint_move":
            for target in s["positions"]:
                ax.axhline(target, color="gray", alpha=0.25, lw=0.8, ls=":")
    t_abort = bag.first_marker("abort_request")
    if t_abort:
        ax.axvline(t_abort - t0, color="tab:red", lw=1.2, label="E-STOP")
    ax.set_xlabel("t [s]"); ax.set_ylabel("posición articular [rad]")
    ax.set_title(title); ax.legend(); ax.grid(alpha=0.3)
    fig.savefig(out, dpi=150, bbox_inches="tight"); plt.close(fig)


def fig_estop_timeline(bag: BagData, out: Path, title: str) -> None:
    if not bag.cmd_vel:
        return
    t_abort = bag.first_marker("abort_request")
    t0 = bag.cmd_vel[0][0]
    fig, ax = plt.subplots(figsize=(8, 3.5))
    ax.step([t - t0 for t, *_ in bag.cmd_vel], [lin for _, lin, _ in bag.cmd_vel],
            where="post", lw=1.2, label="v lineal comandada [m/s]")
    if t_abort:
        ax.axvline(t_abort - t0, color="tab:red", lw=1.4, label="abort_request")
    ax.set_xlabel("t [s]"); ax.set_ylabel("v [m/s]")
    ax.set_title(title); ax.legend(); ax.grid(alpha=0.3)
    fig.savefig(out, dpi=150, bbox_inches="tight"); plt.close(fig)


def fig_spaghetti(run_dirs: list[Path], steps: list[dict], out: Path, title: str) -> None:
    """All-runs trajectory overlay: the repeatability figure."""
    fig, ax = plt.subplots(figsize=(6, 6))
    ideal = ideal_polyline(steps)
    ax.plot([p[0] for p in ideal], [p[1] for p in ideal], "k--", lw=2,
            marker="o", ms=4, label="Ideal", zorder=5)
    for rd in run_dirs:
        try:
            bag = BagData(rd / "bag")
        except Exception:  # noqa: BLE001
            continue
        if not bag.pose:
            continue
        t_req = bag.first_marker("exec_request") or bag.pose[0][0]
        p0 = bag.pose_at(t_req) or bag.pose[0]
        c, s0 = math.cos(-p0[3]), math.sin(-p0[3])
        xs, ys = [], []
        for _, x, y, _ in bag.pose:
            dx, dy = x - p0[1], y - p0[2]
            xs.append(dx * c - dy * s0); ys.append(dx * s0 + dy * c)
        ax.plot(xs, ys, color="tab:blue", alpha=0.18, lw=0.8)
    ax.set_xlabel("x [m]"); ax.set_ylabel("y [m]")
    ax.set_title(title); ax.legend(); ax.axis("equal"); ax.grid(alpha=0.3)
    fig.savefig(out, dpi=150, bbox_inches="tight"); plt.close(fig)


def fig_hist(series: pd.Series, out: Path, title: str, xlabel: str) -> None:
    data = series.dropna()
    if data.empty:
        return
    fig, ax = plt.subplots(figsize=(6, 3.5))
    ax.hist(data, bins=min(15, max(5, len(data) // 2)), color="tab:blue",
            alpha=0.8, edgecolor="white")
    ax.axvline(data.mean(), color="tab:red", lw=1.4,
               label=f"μ = {data.mean():.2f}, σ = {data.std():.2f}")
    ax.set_xlabel(xlabel); ax.set_ylabel("corridas")
    ax.set_title(title); ax.legend(); ax.grid(alpha=0.3)
    fig.savefig(out, dpi=150, bbox_inches="tight"); plt.close(fig)


# ─── Scenario-level orchestration ────────────────────────────────────────────


def analyze_scenario(scenario_dir: Path, results_dir: Path) -> None:
    run_dirs = sorted(d for d in scenario_dir.iterdir() if (d / "metadata.json").exists())
    if not run_dirs:
        return
    name = f"{scenario_dir.parent.name}/{scenario_dir.name}"
    print(f"── {name}: {len(run_dirs)} runs")

    rows = []
    for rd in run_dirs:
        try:
            if row := analyze_run(rd):
                rows.append(row)
        except Exception as exc:  # noqa: BLE001
            print(f"   {rd.name}: analysis failed: {exc}")
    if not rows:
        return

    out = results_dir / scenario_dir.parent.name / scenario_dir.name
    out.mkdir(parents=True, exist_ok=True)

    df = pd.DataFrame(rows).set_index("run").sort_index()
    df.to_csv(out / "summary.csv")

    numeric = df.select_dtypes("number")
    aggregate = {
        col: {"mean": round(float(numeric[col].mean()), 4),
              "std": round(float(numeric[col].std()), 4),
              "min": round(float(numeric[col].min()), 4),
              "max": round(float(numeric[col].max()), 4),
              "n": int(numeric[col].count())}
        for col in numeric.columns
    }
    aggregate["markers_complete_rate"] = round(float(df["markers_complete"].mean()), 4)
    (out / "aggregate.json").write_text(json.dumps(aggregate, indent=2))

    # Representative-run figures (run_001) + aggregates
    meta = json.loads((run_dirs[0] / "metadata.json").read_text())
    steps = meta["program"]["steps"]
    bag = BagData(run_dirs[0] / "bag")
    is_arm = any(s["type"] == "joint_move" for s in steps)
    is_estop = meta.get("abort") is not None

    if is_arm:
        fig_joints(bag, steps, out / "joints_timeseries.png", name)
    elif is_estop:
        fig_estop_timeline(bag, out / "estop_timeline.png", name)
    else:
        fig_trajectory(bag, steps, out / "trajectory_xy.png", name)
        fig_timeseries(bag, out / "timeseries.png", name)

    fig_hist(df.get("lat_total_ms", pd.Series(dtype=float)),
             out / "latency_hist.png", f"{name} — latencia sistémica", "L_s [ms]")
    if "estop_cmd_ms" in df:
        fig_hist(df["estop_cmd_ms"], out / "estop_hist.png",
                 f"{name} — reacción E-STOP", "t [ms]")
    if scenario_dir.name.startswith("S3") and not is_arm:
        fig_spaghetti(run_dirs, steps, out / "spaghetti_xy.png",
                      f"{name} — {len(run_dirs)} corridas superpuestas")

    lat = aggregate.get("lat_total_ms", {})
    print(f"   L_s: μ={lat.get('mean')}ms σ={lat.get('std')}ms | "
          f"markers OK: {aggregate['markers_complete_rate']*100:.0f}% | → {out}")


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("data", help="campaign data directory (runner.py --out)")
    ap.add_argument("--results", default="results")
    args = ap.parse_args()

    data = Path(args.data)
    results = Path(args.results)
    for platform_dir in sorted(p for p in data.iterdir() if p.is_dir()):
        for scenario_dir in sorted(s for s in platform_dir.iterdir() if s.is_dir()):
            analyze_scenario(scenario_dir, results)
    print("Análisis completo.")


if __name__ == "__main__":
    main()
