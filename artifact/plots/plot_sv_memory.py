#!/usr/bin/env python3
"""Generate a single SV-memory plot up to m = 2^22."""

import csv
from pathlib import Path

import matplotlib as mpl
import matplotlib.pyplot as plt
import numpy as np
from matplotlib.ticker import FuncFormatter, LogLocator, NullFormatter

mpl.rcParams.update(
    {
        "font.size": 21,
        "axes.labelsize": 23,
        "axes.titlesize": 23,
        "xtick.labelsize": 19,
        "ytick.labelsize": 19,
        "legend.fontsize": 19,
        "pdf.fonttype": 42,
        "ps.fonttype": 42,
        # "mathtext.fontset": "dejavusans",
        "axes.linewidth": 1.1,
        "lines.linewidth": 2.0,
        "lines.markersize": 11,
        "xtick.direction": "in",
        "ytick.direction": "in",
        "xtick.major.size": 6,
        "ytick.major.size": 6,
        "xtick.minor.size": 3.5,
        "ytick.minor.size": 3.5,
    }
)

ROOT = Path(__file__).resolve().parent
DATA = ROOT / "data-memory"
OUT = ROOT / "sv_memory_up_to_2pow22.pdf"

MIN_EXP = 0
MAX_EXP = 22
EXP_STEP = 2
ANNOTATED_EXPS = (20, 22)

SERIES = [
    {
        "csv": DATA / "cwkpir_n2_13.csv",
        "label": r"CWKPIR ($N$=2^13)",
        "color": "#d62728",
        "ls": "-",
        "marker": "o",
        "annotate": True,
    },
    {
        "csv": DATA / "cwkpir_n2_14.csv",
        "label": r"CWKPIR ($N$=2^14)",
        "color": "#d62728",
        "ls": "--",
        "marker": "s",
        "annotate": True,
    },
    {
        "csv": DATA / "tcakpir.csv",
        "label": "TCAKPIR",
        "color": "#1f77b4",
        "ls": "-.",
        "marker": "^",
    },
    {
        "csv": DATA / "tcakpir_opt.csv",
        "label": "TCAKPIR+",
        "color": "#1f77b4",
        "ls": ":",
        "marker": "+",
        "mew": 2.5,
        "annotate": True,
        "annotation_offset": (-2, 2),
    },
    {
        "csv": DATA / "tcwkpir.csv",
        "label": "TCWKPIR",
        "color": "#1f77b4",
        "ls": "-",
        "marker": "v",
        "annotate": True,
        "annotation_offset": (-2, 2),
    },
]


def pow2_formatter(x, _pos):
    if x <= 0:
        return ""
    exp = int(round(np.log2(x)))
    if abs(x - 2**exp) / x > 1e-6:
        return ""
    return rf"$2^{{{exp}}}$"


def load_points(csv_path):
    xs = []
    ys = []
    with csv_path.open(newline="") as f:
        for row in csv.DictReader(f):
            exp = int(row["kw_bitlen"])
            if exp < MIN_EXP or exp > MAX_EXP or exp % EXP_STEP:
                continue
            xs.append(2**exp)
            ys.append(float(row["total_sv_mb"]))
    if not xs:
        raise ValueError(f"No rows in exponent range for {csv_path}")
    return np.array(xs, dtype=float), np.array(ys, dtype=float)


def format_memory(value_mb):
    if value_mb >= 1024**2:
        coefficient, exponent = f"{value_mb:.1e}".split("e")
        return rf"${coefficient} \times 10^{{{int(exponent)}}}$ MB"
    if value_mb >= 1024:
        return f"{round(value_mb / 1024)} GB"
    return f"{round(value_mb)} MB"


def annotate_values(ax, spec, x, y):
    if not spec.get("annotate"):
        return
    for exp in ANNOTATED_EXPS:
        index = int(np.where(x == 2**exp)[0][0])
        ax.annotate(
            format_memory(y[index]),
            (x[index], y[index]),
            xytext=spec.get("annotation_offset", (-6, 6)),
            textcoords="offset points",
            ha="right",
            va="bottom",
            color=spec["color"],
            fontsize=14,
            zorder=4,
        )


def main():
    fig, ax = plt.subplots(figsize=(8.0, 5.1))

    y_all = []
    handles = []
    for spec in SERIES:
        x, y = load_points(spec["csv"])
        y_all.append(y)
        marker_face = "white" if spec["marker"] != "+" else spec["color"]
        marker_edge_width = spec.get("mew", 1.8)
        (h,) = ax.plot(
            x,
            y,
            color=spec["color"],
            linestyle=spec["ls"],
            marker=spec["marker"],
            markerfacecolor=marker_face,
            markeredgecolor=spec["color"],
            markeredgewidth=marker_edge_width,
            label=spec["label"],
            zorder=3,
        )
        handles.append(h)

    y_concat = np.concatenate(y_all)
    y_min = float(y_concat.min())
    y_max = float(y_concat.max())

    ax.set_xscale("log", base=2)
    ax.set_yscale("log")
    ax.set_xlim(2 ** (MIN_EXP - 0.55), 2 ** (MAX_EXP + 0.55))
    ax.set_ylim(10 ** (np.log10(y_min) - 0.4), 10 ** (np.log10(y_max) + 0.6))

    major_exps = list(range(MIN_EXP, MAX_EXP + 1, EXP_STEP))
    ax.set_xticks([2**e for e in major_exps])
    ax.set_xticks([2**e for e in range(MIN_EXP, MAX_EXP + 1)], minor=True)
    ax.xaxis.set_major_formatter(FuncFormatter(pow2_formatter))
    ax.xaxis.set_minor_formatter(NullFormatter())
    ax.yaxis.set_major_locator(LogLocator(base=10, numticks=8))
    ax.yaxis.set_minor_formatter(NullFormatter())

    ax.grid(True, which="major", color="#d4d4d4", linewidth=0.8, zorder=0)
    ax.grid(True, which="minor", color="#ececec", linewidth=0.5, zorder=0)
    ax.set_axisbelow(True)

    ax.set_xlabel("Number of Entries (m)")
    ax.set_ylabel("SV Memory (MB)")

    fig.legend(
        handles,
        [s["label"] for s in SERIES],
        loc="upper center",
        ncol=3,
        frameon=True,
        fancybox=False,
        edgecolor="#b0b0b0",
        borderpad=0.35,
        handlelength=1.8,
        columnspacing=1.0,
        handletextpad=0.4,
        labelspacing=0.25,
        bbox_to_anchor=(0.5, 0.985),
    )

    fig.subplots_adjust(left=0.13, right=0.97, bottom=0.20, top=0.74)
    fig.savefig(OUT, bbox_inches="tight", pad_inches=0.04)
    print(f"wrote {OUT}")


if __name__ == "__main__":
    main()