"""Summarize exact carry transitions from carry_sweep_results.json (no GIS computations)."""
import json
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent


def carried(fmt, row):
    tokens = row['text'].split()
    if fmt == 'DMS_1':
        component = tokens[2 if row['axis'] == 'y' else 5]
    else:
        component = tokens[1 if row['axis'] == 'y' else 3]
    return float(component[:-1]) == 0.0


def main():
    results = json.loads((HERE / (sys.argv[1] if len(sys.argv) > 1 else 'carry_sweep_results.json')).read_text())
    for result in results:
        fmt = result['format']
        groups = sorted(set(row['group'] for row in result['values']))
        for group in groups:
            rows = [row for row in result['values'] if row['group'] == group]
            bits = [carried(fmt, row) for row in rows]
            transition = next((i for i, bit in enumerate(bits) if bit), None)
            assert transition and not any(bits[:transition]) and all(bits[transition:]), (fmt, group)
            before, after = rows[transition-1], rows[transition]
            print(fmt, group, before['minute_units'], after['minute_units'],
                  before['text'], '->', after['text'])


if __name__ == '__main__':
    main()
