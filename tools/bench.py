#!/usr/bin/env python3
"""QuickMD の起動時間を測る。

プロセスを立てた時刻から、本文が画面に出るまでを何回か測り、中央値で比べる。
アプリ側は --timing でその内訳を JSON に書くので、どこで時間を使っているかまで分かる。

    python3 tools/bench.py examples/basic.md -n 10            開発用のビルドを測る
    python3 tools/bench.py examples/basic.md -n 10 --release  配布用のビルドを測る
    python3 tools/bench.py examples/basic.md --compare        OS の標準アプリとも比べる
"""

import argparse
import json
import os
import platform
import statistics
import subprocess
import sys
import tempfile
import time

ROOT = os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))), "native")
IS_WIN = platform.system() == "Windows"


def exe_path(release):
    name = "quickmd-native.exe" if IS_WIN else "quickmd-native"
    p = os.path.join(ROOT, "target", "release" if release else "debug", name)
    if not os.path.exists(p):
        raise SystemExit(f"{p} がありません。先にビルドしてください。")
    return p


def one_run(exe, target, timeout=30):
    with tempfile.TemporaryDirectory() as tmp:
        out = os.path.join(tmp, "timing.json")
        argv = [exe, target, "--timing", out, "--exit-after-ready"]
        t0 = time.perf_counter()
        proc = subprocess.Popen(argv, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        end = t0 + timeout
        while time.perf_counter() < end:
            if os.path.exists(out) and os.path.getsize(out) > 0:
                wall = (time.perf_counter() - t0) * 1000
                time.sleep(0.02)
                try:
                    data = json.load(open(out, encoding="utf-8"))
                except json.JSONDecodeError:
                    time.sleep(0.05)
                    data = json.load(open(out, encoding="utf-8"))
                try:
                    proc.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    proc.terminate()
                return wall, data
            if proc.poll() is not None:
                break
            time.sleep(0.005)
        proc.terminate()
        raise SystemExit("起動しましたが、時間の記録が出てきませんでした。")


def summarize(values):
    return {
        "回数": len(values),
        "最小": round(min(values), 1),
        "中央": round(statistics.median(values), 1),
        "平均": round(statistics.mean(values), 1),
        "最大": round(max(values), 1),
    }


def compare_builtin(target):
    """OS の標準のテキストアプリが立ち上がるまでを、同じ方法で測る。"""
    if IS_WIN:
        argv = ["notepad.exe", target]
        name = "メモ帳"
    elif platform.system() == "Darwin":
        argv = ["open", "-a", "TextEdit", target]
        name = "テキストエディット"
    else:
        return None
    t0 = time.perf_counter()
    subprocess.run(argv, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    return name, (time.perf_counter() - t0) * 1000


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("file")
    ap.add_argument("-n", type=int, default=8)
    ap.add_argument("--release", action="store_true")
    ap.add_argument("--warmup", type=int, default=2)
    ap.add_argument("--compare", action="store_true")
    ap.add_argument("--json", help="結果を書き出す先")
    args = ap.parse_args()

    exe = exe_path(args.release)
    target = os.path.abspath(args.file)

    for _ in range(args.warmup):
        one_run(exe, target)

    walls, totals, marks_all = [], [], []
    for i in range(args.n):
        wall, data = one_run(exe, target)
        walls.append(wall)
        totals.append(data["total_ms"])
        marks_all.append({m["name"]: m["ms"] for m in data["marks"]})
        print(f"  {i + 1:2d} 回目: 全体 {wall:7.1f} ms（アプリの中 {data['total_ms']:6.1f} ms）")

    print()
    print("=== 起動から本文が出るまで（ミリ秒） ===")
    print("プロセスを立ててから:", summarize(walls))
    print("main に入ってから  :", summarize(totals))

    names = []
    for m in marks_all:
        for k in m:
            if k not in names:
                names.append(k)
    print()
    print("=== 内訳（中央値・ミリ秒） ===")
    prev = 0.0
    order = sorted(names, key=lambda n: statistics.median([m.get(n, 0) for m in marks_all if n in m]))
    for n in order:
        vals = [m[n] for m in marks_all if n in m]
        med = statistics.median(vals)
        print(f"  {n:24s} {med:7.1f}   （その前から +{med - prev:6.1f}）")
        prev = med

    if args.compare:
        r = compare_builtin(target)
        if r:
            print()
            print(f"=== 比較 === {r[0]}: {r[1]:.1f} ms（コマンドが返るまで）")

    if args.json:
        json.dump(
            {"wall": walls, "total": totals, "marks": marks_all},
            open(args.json, "w", encoding="utf-8"),
            ensure_ascii=False,
            indent=2,
        )


if __name__ == "__main__":
    main()
