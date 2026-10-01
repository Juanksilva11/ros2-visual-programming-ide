#!/usr/bin/env python3
"""Estadísticos adicionales para el Capítulo 6 (rigor de nivel laureado).

Calcula sobre los summary.csv de la campaña:
  - mediana + RIC + máx de L_s por escenario (para la tabla maestra)
  - correlación de Spearman (rho, p) entre complejidad y latencia
  - intervalos de confianza al 95 % (media -> t-Student; mediana -> bootstrap)
  - contraste de E_p Turtlesim vs Waffle (descomposición de fuentes de error)

Solo lee datos ya existentes; no ejecuta experimentos.
"""
import csv
import statistics as st
from pathlib import Path

import numpy as np
from scipy import stats

RESULTS = Path("results")
NB = {"S1_min": 1, "S2_line_return": 3, "S3_square": 8, "S4_long": 12}
SCEN_ALL = list(NB) + ["S5_estop", "S6_arm_cycle", "S7_arm_estop"]


def col(plat, scen, c):
    p = RESULTS / plat / scen / "summary.csv"
    if not p.exists():
        return []
    rows = list(csv.DictReader(open(p)))
    return [float(r[c]) for r in rows if r.get(c) not in (None, "", "None")]


def iqr(v):
    q1, q3 = np.percentile(v, [25, 75])
    return q3 - q1


def ci_mean(v, conf=0.95):
    n = len(v)
    m = st.mean(v)
    se = st.stdev(v) / np.sqrt(n)
    h = stats.t.ppf(0.5 + conf / 2, n - 1) * se
    return m, m - h, m + h


def ci_median_boot(v, conf=0.95, B=10000, seed=1):
    rng = np.random.default_rng(seed)
    v = np.array(v)
    meds = [np.median(rng.choice(v, size=len(v), replace=True)) for _ in range(B)]
    lo, hi = np.percentile(meds, [(1 - conf) / 2 * 100, (1 + conf) / 2 * 100])
    return np.median(v), lo, hi


print("=" * 70)
print("1) MEDIANA / RIC / MÁX de L_s por escenario  (para la tabla maestra)")
print("=" * 70)
for plat in ["turtlesim", "waffle", "arm"]:
    for scen in SCEN_ALL:
        v = col(plat, scen, "lat_total_ms")
        if not v:
            continue
        print(f"  {plat:10s} {scen:15s} n={len(v):2d}  "
              f"mediana={np.median(v):.3f}  RIC={iqr(v):.3f}  máx={max(v):.2f}  "
              f"(media={st.mean(v):.3f})")

print()
print("=" * 70)
print("2) SPEARMAN  complejidad (nº bloques) vs L_s   [S1-S4, por plataforma]")
print("=" * 70)
for plat in ["turtlesim", "waffle"]:
    xs, ys = [], []
    for scen, nb in NB.items():
        for val in col(plat, scen, "lat_total_ms"):
            xs.append(nb)
            ys.append(val)
    rho, p = stats.spearmanr(xs, ys)
    print(f"  {plat:10s} n={len(xs)}  rho={rho:+.3f}  p={p:.4f}  "
          f"-> {'SIN correlación significativa' if p > 0.05 else 'correlación significativa'}")

print()
print("=" * 70)
print("3) INTERVALOS DE CONFIANZA 95 %")
print("=" * 70)
print("  -- Reacción E-STOP (media, IC t-Student) por plataforma --")
for plat, scen in [("turtlesim", "S5_estop"), ("waffle", "S5_estop"), ("arm", "S7_arm_estop")]:
    v = col(plat, scen, "estop_cmd_ms")
    if v:
        m, lo, hi = ci_mean(v)
        print(f"     {plat:10s} n={len(v):2d}  media={m:.3f} ms  IC95=[{lo:.3f}, {hi:.3f}]  máx={max(v):.3f}")
print("  -- Error posicional E_p, cuadrado S3 (media, IC t-Student) --")
for plat in ["turtlesim", "waffle"]:
    v = col(plat, "S3_square", "ep_final_m")
    if v:
        m, lo, hi = ci_mean(v)
        print(f"     {plat:10s} n={len(v):2d}  media={m*100:.2f} cm  IC95=[{lo*100:.2f}, {hi*100:.2f}] cm  σ={st.stdev(v)*100:.2f} cm")
print("  -- L_s mediana (bootstrap IC) para S3 --")
for plat in ["turtlesim", "waffle"]:
    v = col(plat, "S3_square", "lat_total_ms")
    if v:
        med, lo, hi = ci_median_boot(v)
        print(f"     {plat:10s} n={len(v):2d}  mediana={med:.3f} ms  IC95=[{lo:.3f}, {hi:.3f}]")

print()
print("=" * 70)
print("4) DESCOMPOSICIÓN DE E_p: Turtlesim (sin física) vs Waffle (con física)")
print("=" * 70)
for scen in ["S2_line_return", "S3_square", "S4_long"]:
    vt = col("turtlesim", scen, "ep_final_m")
    vw = col("waffle", scen, "ep_final_m")
    if vt and vw:
        print(f"  {scen:15s}  Turtlesim E_p={st.mean(vt)*100:.2f}cm (σ={st.stdev(vt)*100:.2f})  "
              f"Waffle E_p={st.mean(vw)*100:.2f}cm (σ={st.stdev(vw)*100:.2f})")
print("  -- Error articular del manipulador (S6) --")
for c in ["ep_joint_max_rad", "ep_joint_mean_rad"]:
    v = col("arm", "S6_arm_cycle", c)
    if v:
        print(f"     {c:20s} media={st.mean(v):.4f} rad  máx={max(v):.4f} rad")
