#!/usr/bin/env python3
"""Diagnóstico de integridad de los 360 bags de la campaña.

Para cada corrida verifica (leyendo solo el tópico de marcadores):
  - presencia del marcador exec_request (raíz del problema)
  - nº de STEP_START capturados vs esperados
  - presencia de PROGRAM_FINISH / PROGRAM_ABORT
Reporta, por escenario, cuántas corridas están completas.
"""
import json
from pathlib import Path

import rosbag2_py
from rclpy.serialization import deserialize_message
from std_msgs.msg import String

DATA = Path("data")
MARKER = "/webapp_ros/markers"

# nº de bloques (STEP_START esperados) por escenario
STEPS = {
    "S1_min": 1, "S2_line_return": 3, "S3_square": 8, "S4_long": 12,
    "S5_estop": 1, "S6_arm_cycle": 5, "S7_arm_estop": 1,
}
ABORT = {"S5_estop", "S7_arm_estop"}


def read_markers(bag_dir):
    reader = rosbag2_py.SequentialReader()
    reader.open(rosbag2_py.StorageOptions(uri=str(bag_dir)),
                rosbag2_py.ConverterOptions("", ""))
    types = {t.name: t.type for t in reader.get_all_topics_and_types()}
    events = []
    while reader.has_next():
        topic, raw, t = reader.read_next()
        if topic != MARKER:
            continue
        msg = deserialize_message(raw, String)
        d = json.loads(msg.data)
        ev = d["event"]
        if ev == "system_event":
            ev = d.get("payload", {}).get("event_type", "system_event")
        events.append(ev)
    return events


def main():
    total_ok = total = 0
    print(f"{'escenario':28s} {'ok/tot':>8s}  detalle de fallos")
    print("-" * 78)
    for plat in ["turtlesim", "waffle", "arm"]:
        for scen in STEPS:
            base = DATA / plat / scen
            if not base.exists():
                continue
            run_dirs = sorted(d for d in base.iterdir() if (d / "bag").exists())
            ok = 0
            fails = []
            for rd in run_dirs:
                try:
                    evs = read_markers(rd / "bag")
                except Exception as e:  # noqa: BLE001
                    fails.append(f"{rd.name}(err)")
                    continue
                has_req = "exec_request" in evs
                n_start = evs.count("STEP_START")
                has_end = ("PROGRAM_FINISH" in evs) or ("PROGRAM_ABORT" in evs)
                has_abort = "abort_request" in evs
                complete = has_req and n_start >= STEPS[scen] and has_end
                if scen in ABORT:
                    complete = has_req and has_abort and has_end
                if complete:
                    ok += 1
                else:
                    reason = []
                    if not has_req:
                        reason.append("sin exec_request")
                    if n_start < STEPS[scen]:
                        reason.append(f"{n_start}/{STEPS[scen]} steps")
                    if scen in ABORT and not has_abort:
                        reason.append("sin abort")
                    if not has_end:
                        reason.append("sin fin")
                    fails.append(f"{rd.name}({','.join(reason)})")
            total_ok += ok
            total += len(run_dirs)
            key = f"{plat}/{scen}"
            detail = "" if not fails else "  ".join(fails[:3]) + (" ..." if len(fails) > 3 else "")
            print(f"{key:28s} {ok:3d}/{len(run_dirs):<3d}  {detail}")
    print("-" * 78)
    print(f"TOTAL corridas completas: {total_ok}/{total}")


if __name__ == "__main__":
    main()
