# PIR Plot Scripts

Python scripts and CSV inputs for reproducing Figures 5 and 6.

## Files

- `plot_all_four_comm.py`: Figure 6 (communication costs)
- `plot_sv_memory.py`: Figure 5 (selection-vector memory)
- `data/`: CSV inputs for Figure 6
- `data-memory/`: CSV inputs for Figure 5

## Dependencies

```bash
pip install matplotlib numpy
```

## Run

From this directory:

```bash
python plot_all_four_comm.py
python plot_sv_memory.py
```

The scripts generate `all_four_comm_one_plot.pdf` and `sv_memory_up_to_2pow22.pdf`, respectively.