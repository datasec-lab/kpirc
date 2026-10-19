#!/usr/bin/env python3
"""Regenerate all_four_comm_one_plot.pdf. Missing CSVs stay as placeholders."""

import csv
from pathlib import Path

import matplotlib as mpl
import matplotlib.pyplot as plt
import numpy as np
from matplotlib.ticker import FuncFormatter, LogLocator, NullFormatter, NullLocator

# Original figure used Matplotlib 3.6.3 + embedded DejaVu Sans (TrueType).
mpl.rcParams.update(
    {
        # "font.family": "DejaVu Sans",
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
OUT = ROOT / "all_four_comm_one_plot.pdf"
DATA = ROOT / "data"

# Plot number of entries from 2^0 through 2^20 in 4-exponent increments,
# and entry sizes from 2^0 through 2^12 in 2-exponent increments.
M_EXP_LO, M_EXP_HI, M_EXP_STEP = 0, 20, 4
E_EXP_LO, E_EXP_HI, E_EXP_STEP = 0, 12, 2
M_LO, M_HI = 2**M_EXP_LO, 2**M_EXP_HI
E_LO, E_HI = 2**E_EXP_LO, 2**E_EXP_HI
ENTRY_FIXED = 8
M_FIXED = 2**16

SERIES = [
    {
        "csv": DATA / "cwkpir_n2_13.csv",
        "color": "#d62728",
        "ls": "-",
        "marker": "o",
        "label": r"CWKPIR ($N=2^{13}$)",
        "placeholder": {"up_m": 220, "dn_m": 105, "up_e": 220, "dn_e": 105},
    },
    {
        "csv": DATA / "cwkpir_n2_14.csv",
        "color": "#d62728",
        "ls": "--",
        "marker": "s",
        "label": r"CWKPIR ($N=2^{14}$)",
        "placeholder": {"up_m": 900, "dn_m": 225, "up_e": 900, "dn_e": 225},
    },
    {
        "csv": DATA / "chalamet_q32_n1774.csv",
        "color": "#1f77b4",
        "ls": "-.",
        "marker": "^",
        "label": r"ChalametPIR ($q = 2^{32}$, $n=1774$)",
        "placeholder": {"up_m": "grow", "dn_m": 8, "up_e": 310, "dn_e": "e"},
    },
    {
        "csv": DATA / "chalamet_q64_n2048.csv",
        "color": "#1f77b4",
        "ls": ":",
        "marker": "D",
        "label": r"ChalametPIR ($q = 2^{64}$, $n=2048$)",
        "placeholder": {"up_m": "grow64", "dn_m": 18, "up_e": 620, "dn_e": "e64"},
    },
    {
        "csv": DATA / "tcakpir.csv",
        "color": "black",
        "ls": "--",
        "marker": "v",
        "label": "TCAKPIR",
        "placeholder": {"up_m": "tca", "dn_m": 4, "up_e": 600, "dn_e": "tca_e"},
    },
    {
        "csv": DATA / "tcwkpir.csv",
        "color": "black",
        "ls": "-",
        "marker": "P",
        "label": "TCWKPIR",
        "placeholder": {"up_m": "tcw", "dn_m": 2, "up_e": 40, "dn_e": "tcw_e"},
    },
]

M_GRID = 2 ** np.arange(M_EXP_LO, M_EXP_HI + 1, M_EXP_STEP)
E_GRID = 2 ** np.arange(E_EXP_LO, E_EXP_HI + 1, E_EXP_STEP)


def _const(val, x):
    return np.full_like(x, val, dtype=float)


def _placeholder_xy(kind, spec):
    if kind in ("up_m", "dn_m"):
        x = M_GRID
    else:
        x = E_GRID
    val = spec[kind]
    if val == "grow":
        y = 0.08 * x**0.75
    elif val == "grow64":
        y = 0.12 * x**0.82
    elif val == "tca":
        y = 0.05 * x**0.88
    elif val == "tcw":
        y = 0.20 * x**0.45
    elif val == "e":
        y = 0.9 * x
    elif val == "e64":
        y = 1.6 * x
    elif val == "tca_e":
        y = 4 + 0.004 * np.maximum(x - 256, 0) ** 1.15
    elif val == "tcw_e":
        y = 2 + 0.003 * np.maximum(x - 256, 0) ** 1.15
    else:
        y = _const(val, x)
    return x.astype(float), y.astype(float)


def payload_b(row):
    """Payload size. TCA/TCW store encoded size (payload + keyword bytes)."""
    encoded = row["entry_size_b"]
    if row["scheme"] in ("TCAKPIR", "TCWKPIR"):
        return encoded - row["max_keyword_bitlen"] / 8.0
    return encoded


def load_csv(path):
    rows = []
    with path.open(newline="") as f:
        for row in csv.DictReader(f):
            required = (
                "scheme",
                "max_keyword_bitlen",
                "num_keywords",
                "entry_size_b",
                "upload_kb",
                "download_kb",
            )
            if any(not row.get(field) for field in required):
                continue
            rec = {
                "scheme": row["scheme"],
                "max_keyword_bitlen": int(row["max_keyword_bitlen"]),
                "num_keywords": int(row["num_keywords"]),
                "entry_size_b": float(row["entry_size_b"]),
                "upload_kb": float(row["upload_kb"]),
                "download_kb": float(row["download_kb"]),
            }
            rec["payload_b"] = payload_b(rec)
            rows.append(rec)
    return rows


def slice_vs_m_fixed_entry(rows):
    """Top row: fix |Entry| and keep every number-of-entry point at that size."""
    by_m = {}
    for row in rows:
        if row["num_keywords"] in M_GRID:
            by_m.setdefault(row["num_keywords"], []).append(row)
    pts = [
        min(by_m[m], key=lambda row: abs(row["payload_b"] - ENTRY_FIXED))
        for m in sorted(by_m)
    ]
    pts.sort(key=lambda r: r["num_keywords"])
    x = np.array([r["num_keywords"] for r in pts], dtype=float)
    up = np.array([r["upload_kb"] for r in pts], dtype=float)
    dn = np.array([r["download_kb"] for r in pts], dtype=float)
    return x, up, dn


def slice_vs_entry_fixed_m(rows):
    """Bottom row: vary raw |Entry| byte length at m = 2^16."""
    pts = [
        r
        for r in rows
        if r["num_keywords"] == M_FIXED
        and any(abs(r["entry_size_b"] - entry_size) < 1e-9 for entry_size in E_GRID)
    ]
    pts.sort(key=lambda r: r["entry_size_b"])
    x = np.array([r["entry_size_b"] for r in pts], dtype=float)
    up = np.array([r["upload_kb"] for r in pts], dtype=float)
    dn = np.array([r["download_kb"] for r in pts], dtype=float)
    return x, up, dn


def data_ylim(curves, log=False, pad=0.12, bottom_pad=0.0):
    ys = np.concatenate([y for _, y in curves])
    ymin, ymax = float(ys.min()), float(ys.max())
    if log:
        lo = 10 ** (np.log10(ymin) - pad)
        hi = 10 ** (np.log10(ymax) + pad)
        return lo, hi
    return -ymax * bottom_pad, ymax * (1.0 + pad)


def series_curves(spec):
    path = spec["csv"]
    if path.is_file():
        rows = load_csv(path)
        x_m, up_m, dn_m = slice_vs_m_fixed_entry(rows)
        x_e, up_e, dn_e = slice_vs_entry_fixed_m(rows)
        if len(x_m) == 0 or len(x_e) == 0:
            raise SystemExit(f"no plottable rows in {path}")
        return {
            "up_m": (x_m, up_m),
            "dn_m": (x_m, dn_m),
            "up_e": (x_e, up_e),
            "dn_e": (x_e, dn_e),
        }
    ph = spec["placeholder"]
    return {k: _placeholder_xy(k, ph) for k in ("up_m", "dn_m", "up_e", "dn_e")}


def pow2_formatter(x, _pos):
    if x <= 0:
        return ""
    exp = int(round(np.log2(x)))
    if abs(x - 2**exp) / x > 1e-6:
        return ""
    return rf"$2^{{{exp}}}$"


def plot_series(ax, curves):
    handles = []
    for spec, (x, y) in zip(SERIES, curves):
        (h,) = ax.plot(
            x,
            y,
            color=spec["color"],
            linestyle=spec["ls"],
            marker=spec["marker"],
            markerfacecolor="white",
            markeredgecolor=spec["color"],
            markeredgewidth=2.0,
            label=spec["label"],
            zorder=3,
        )
        handles.append(h)
    return handles


def style_log2_x(ax, major_exp, exp_lo, exp_hi):
    ax.set_xscale("log", base=2)
    ax.set_xticks([2**e for e in major_exp])
    ax.set_xticks([2**e for e in range(exp_lo, exp_hi + 1)], minor=True)
    ax.xaxis.set_major_formatter(FuncFormatter(pow2_formatter))
    ax.xaxis.set_minor_formatter(NullFormatter())


def log_xlim(lo, hi, pad=0.55):
    """Pad a log2 axis so the first/last points are not flush with the spines."""
    return 2 ** (np.log2(lo) - pad), 2 ** (np.log2(hi) + pad)


def add_grid(ax):
    ax.grid(True, which="major", color="#d4d4d4", linewidth=0.8, zorder=0)
    ax.set_axisbelow(True)


def main():
    fig, axes = plt.subplots(
        1,
        2,
        figsize=(14.0, 6.5),
        gridspec_kw={"wspace": 0.30},
    )
    ax_upload, ax_download = axes

    loaded = [series_curves(spec) for spec in SERIES]
    handles = plot_series(ax_upload, [c["up_m"] for c in loaded])
    plot_series(ax_download, [c["dn_e"] for c in loaded])

    ax_upload.set_ylabel("Upload (KB)", labelpad=4)
    ax_download.set_ylabel("Download (KB)", labelpad=4)
    ax_upload.set_xlabel("Number of Entries ($m$)", labelpad=3)
    ax_download.set_xlabel(r"$|Entry|$ (B)", labelpad=3)

    ax_upload.set_yscale("log")
    ax_download.set_yscale("log")
    ax_upload.set_ylim(*data_ylim([c["up_m"] for c in loaded], log=True, pad=0.45))
    ax_download.set_ylim(*data_ylim([c["dn_e"] for c in loaded], log=True, pad=0.65))
    ax_upload.set_xlim(*log_xlim(M_LO, M_HI, pad=0.90))
    ax_download.set_xlim(*log_xlim(E_LO, E_HI))

    style_log2_x(ax_upload, list(range(M_EXP_LO, M_EXP_HI + 1, M_EXP_STEP)), M_EXP_LO, M_EXP_HI)
    style_log2_x(ax_download, list(range(E_EXP_LO, E_EXP_HI + 1, E_EXP_STEP)), E_EXP_LO, E_EXP_HI)

    ax_upload.yaxis.set_major_locator(LogLocator(base=10, numticks=3))
    ax_upload.yaxis.set_minor_locator(NullLocator())
    ax_upload.yaxis.set_minor_formatter(NullFormatter())
    ax_download.yaxis.set_major_locator(LogLocator(base=10, numticks=5))
    ax_download.yaxis.set_minor_locator(NullLocator())
    ax_download.yaxis.set_minor_formatter(NullFormatter())

    ax_upload.yaxis.set_label_coords(-0.12, 0.5)
    ax_download.yaxis.set_label_coords(-0.10, 0.5)

    for ax in axes:
        add_grid(ax)

    # ncol fills down each column: CWKPIR | ChalametPIR | TCAKPIR/TCWKPIR
    fig.legend(
        handles,
        [s["label"] for s in SERIES],
        loc="upper center",
        ncol=3,
        frameon=True,
        fancybox=False,
        edgecolor="#b0b0b0",
        borderpad=0.35,
        handlelength=2.2,
        columnspacing=1.0,
        handletextpad=0.4,
        labelspacing=0.25,
        bbox_to_anchor=(0.5, 0.985),
    )

    fig.subplots_adjust(
        left=0.10, right=0.96, bottom=0.15, top=0.67, wspace=0.30
    )
    fig.savefig(OUT, pad_inches=0.08)
    for spec in SERIES:
        src = spec["csv"].name if spec["csv"].is_file() else "placeholder"
        print(f"{spec['label']}: {src}")
    print(f"wrote {OUT}")


if __name__ == "__main__":
    main()
