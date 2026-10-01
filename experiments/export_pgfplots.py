#!/usr/bin/env python3
"""Exporta los datos de la campaña a archivos .dat para pgfplots.

Reutiliza el lector de bags de analyze.py y los summary.csv de results/.
Genera los 18 archivos que leen las gráficas del Capítulo 6 de la tesis:
  - trayectorias (ideal, real, espagueti) por escenario
  - series temporales (pose, articulaciones, velocidad E-STOP)
  - columnas escalares (latencia, E-STOP) para histogramas/boxplot/CDF
  - tablas agregadas con mediana y RIC (Ls vs complejidad, latencia de la
    plataforma vs respuesta física del simulador)

Uso (desde experiments/, con ROS 2 activo para rosbag2_py):
  python3 export_pgfplots.py --out <carpeta-de-salida>
"""
import json
import argparse
import math

import numpy as np
from pathlib import Path

from analyze import BagData, ideal_polyline

DATA = Path("data")
# Carpeta de salida de los .dat; se fija con --out (ver main()).
OUT = Path("pgfplots")


def rel_frame(bag):
    """Devuelve (transform) para expresar la odometría en el marco inicial."""
    t_req = bag.first_marker("exec_request") or (bag.pose[0][0] if bag.pose else 0)
    p0 = bag.pose_at(t_req) or (bag.pose[0] if bag.pose else (0, 0, 0, 0))
    c, s = math.cos(-p0[3]), math.sin(-p0[3])

    def tf(x, y):
        dx, dy = x - p0[1], y - p0[2]
        return dx * c - dy * s, dx * s + dy * c
    return tf


def write_xy(path, points):
    with open(path, "w") as f:
        f.write("x y\n")
        for x, y in points:
            f.write(f"{x:.5f} {y:.5f}\n")


def steps_of(run_dir):
    return json.loads((run_dir / "metadata.json").read_text())["program"]["steps"]


def runs(plat, scen):
    base = DATA / plat / scen
    return sorted(d for d in base.iterdir() if (d / "metadata.json").exists()) if base.exists() else []


def down(seq, n=250):
    """Submuestrea una secuencia a lo sumo a n puntos."""
    if len(seq) <= n:
        return seq
    step = len(seq) // n
    return seq[::step]


# ─── Trayectorias XY (ideal, real, espagueti) ────────────────────────────────

def export_trajectory(plat, scen, tag):
    rd = runs(plat, scen)
    if not rd:
        return
    steps = steps_of(rd[0])
    write_xy(OUT / f"{tag}_ideal.dat", ideal_polyline(steps))

    bag = BagData(rd[0] / "bag")
    tf = rel_frame(bag)
    write_xy(OUT / f"{tag}_real.dat", [tf(x, y) for _, x, y, _ in down(bag.pose)])


def export_spaghetti(plat, scen, tag):
    rd = runs(plat, scen)
    if not rd:
        return
    with open(OUT / f"{tag}_spaghetti.dat", "w") as f:
        f.write("x y\n")
        for d in rd:
            try:
                bag = BagData(d / "bag")
            except Exception:
                continue
            if not bag.pose:
                continue
            tf = rel_frame(bag)
            for _, x, y, _ in down(bag.pose, 120):
                rx, ry = tf(x, y)
                f.write(f"{rx:.5f} {ry:.5f}\n")
            # "nan nan" separa corridas: con unbounded coords=jump, pgfplots
            # corta el trazo ahí. Una línea en blanco produce el mismo corte,
            # pero pgfplots emite "Missing character: There is no ` in font
            # nullfont!" por cada una.
            f.write("nan nan\n")


# ─── Series temporales ───────────────────────────────────────────────────────

def export_pose_series(plat, scen, tag):
    rd = runs(plat, scen)
    if not rd:
        return
    bag = BagData(rd[0] / "bag")
    if not bag.pose:
        return
    t0 = bag.pose[0][0]
    with open(OUT / f"{tag}_series.dat", "w") as f:
        f.write("t x y yaw\n")
        for t, x, y, yaw in down(bag.pose):
            f.write(f"{t-t0:.4f} {x:.5f} {y:.5f} {yaw:.5f}\n")


def export_joints(plat, scen, tag):
    rd = runs(plat, scen)
    if not rd:
        return
    bag = BagData(rd[0] / "bag")
    if not bag.joints:
        return
    t0 = bag.joints[0][0]
    with open(OUT / f"{tag}_joints.dat", "w") as f:
        f.write("t q0 q1\n")
        for t, q in down(bag.joints):
            if len(q) >= 2:
                f.write(f"{t-t0:.4f} {q[0]:.5f} {q[1]:.5f}\n")


def export_estop_timeline(plat, scen, tag):
    rd = runs(plat, scen)
    if not rd:
        return
    bag = BagData(rd[0] / "bag")
    if not bag.cmd_vel:
        return
    t0 = bag.cmd_vel[0][0]
    t_abort = bag.first_marker("abort_request")
    with open(OUT / f"{tag}_estopvel.dat", "w") as f:
        f.write("t v\n")
        for t, lin, _ in bag.cmd_vel:
            f.write(f"{t-t0:.4f} {lin:.4f}\n")
    if t_abort:
        (OUT / f"{tag}_abort.dat").write_text(f"{t_abort-t0:.4f}\n")


# ─── Columnas escalares (histograma/boxplot/CDF) ─────────────────────────────

def read_summary(plat, scen):
    import csv
    p = Path("results") / plat / scen / "summary.csv"
    if not p.exists():
        return []
    with open(p) as f:
        return list(csv.DictReader(f))


def export_scalar_column(plat, scen, col, path):
    rows = read_summary(plat, scen)
    vals = [float(r[col]) for r in rows if r.get(col) not in (None, "", "None")]
    with open(OUT / path, "w") as f:
        f.write("v\n")
        for v in vals:
            f.write(f"{v:.5f}\n")
    return vals


def export_cdf(values, path):
    vs = sorted(values)
    n = len(vs)
    with open(OUT / path, "w") as f:
        f.write("v p\n")
        for i, v in enumerate(vs, 1):
            f.write(f"{v:.5f} {i/n:.5f}\n")


def med_iqr(plat, scen, col):
    """Mediana y rango intercuartílico de una métrica sobre las corridas.

    La latencia tiene cola derecha (picos de carga del simulador), por lo
    que la tesis la resume con mediana y RIC (Sección 6.8.1). Percentiles
    con interpolación lineal de numpy, igual que stats_extra.py.
    """
    rows = read_summary(plat, scen)
    vals = [float(r[col]) for r in rows if r.get(col) not in (None, "", "None")]
    if not vals:
        return None, None
    q1, q3 = np.percentile(vals, [25, 75])
    return float(np.median(vals)), float(q3 - q1)


# ─── Programa principal ──────────────────────────────────────────────────────

def main():
    global OUT
    ap = argparse.ArgumentParser(description="Exporta los datos de la campaña a archivos .dat para pgfplots.")
    ap.add_argument("--out", default="pgfplots",
                    help="carpeta de salida (p. ej. la carpeta datos/ de la tesis); por defecto ./pgfplots")
    OUT = Path(ap.parse_args().out)
    OUT.mkdir(parents=True, exist_ok=True)
    # Trayectorias
    export_trajectory("turtlesim", "S3_square", "ts_s3")
    export_spaghetti("turtlesim", "S3_square", "ts_s3")
    export_pose_series("turtlesim", "S3_square", "ts_s3")
    export_trajectory("waffle", "S3_square", "wf_s3")

    # Manipulador y E-STOP
    export_joints("arm", "S6_arm_cycle", "arm_s6")
    export_estop_timeline("waffle", "S5_estop", "wf_s5")

    # Histograma de latencia (S3 turtlesim)
    export_scalar_column("turtlesim", "S3_square", "lat_total_ms", "ts_s3_latency.dat")

    # E-STOP: boxplot y CDF por plataforma
    for plat, scen, tag in [("turtlesim", "S5_estop", "ts"),
                            ("waffle", "S5_estop", "wf"),
                            ("arm", "S7_arm_estop", "arm")]:
        vals = export_scalar_column(plat, scen, "estop_cmd_ms", f"estop_{tag}.dat")
        export_cdf(vals, f"estop_{tag}_cdf.dat")

    # Ls vs complejidad topológica (S1-S4, plataformas móviles)
    nblocks = {"S1_min": 1, "S2_line_return": 3, "S3_square": 8, "S4_long": 12}
    with open(OUT / "latency_vs_complexity.dat", "w") as f:
        f.write("nblocks ts_med ts_iqr wf_med wf_iqr\n")
        for scen, nb in nblocks.items():
            ts_med, ts_iqr = med_iqr("turtlesim", scen, "lat_total_ms")
            wf_med, wf_iqr = med_iqr("waffle", scen, "lat_total_ms")
            f.write(f"{nb} {ts_med:.4f} {ts_iqr:.4f} {wf_med:.4f} {wf_iqr:.4f}\n")

    # Latencia de la plataforma vs respuesta física del simulador
    with open(OUT / "latency_platform_vs_physical.dat", "w") as f:
        f.write("idx ls onset label\n")
        labels = {"turtlesim S1_min": "TS-S1", "turtlesim S3_square": "TS-S3",
                  "waffle S1_min": "WF-S1", "waffle S3_square": "WF-S3",
                  "arm S6_arm_cycle": "ARM-S6"}
        i = 0
        for key, lab in labels.items():
            plat, scen = key.split()
            ls, _ = med_iqr(plat, scen, "lat_total_ms")
            onset, _ = med_iqr(plat, scen, "lat_motion_onset_ms")
            if ls and onset:
                f.write(f"{i} {ls:.4f} {onset:.4f} {lab}\n")
                i += 1

    print("Exportación completa. Archivos en:", OUT)
    for p in sorted(OUT.glob("*.dat")):
        print("  ", p.name, f"({p.stat().st_size} B)")


if __name__ == "__main__":
    main()
