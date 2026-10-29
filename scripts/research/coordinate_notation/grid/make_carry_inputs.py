"""Compact carry transitions plus independent geographic/precision holdouts."""
import csv
import random
from pathlib import Path

ROOT = Path(__file__).resolve().parents[4]
LETTERS = 'ABCDEFGHJKLMNPQRSTUVWXYZ'


def coord(x, xm, y, ym):
    a, b = x + 180, y + 90
    return f'{LETTERS[a//15]}{LETTERS[b//15]}{LETTERS[a%15]}{LETTERS[b%15]}{xm:09}{ym:09}'


def main():
    configs = [(0, 45, 'x', False), (15, 45, 'x', False),
               (151, 45, 'x', False), (-16, 45, 'x', True),
               (15, 45, 'y', False), (15, -34, 'y', True)]
    rows = []
    for x, y, axis, negative in configs:
        for value in [49998982, 49998983, 49998984, 49998985,
                      599939002, 599939003, 599939004, 599939005]:
            minute = 600000000 - value if negative else value
            xm, ym = (minute, 100000000) if axis == 'x' else (100000000, minute)
            rows.append(coord(x, xm, y, ym))
    rng = random.Random(47321)
    minutes = [0, 10000, 9998983, 9998984, 49998983, 49998984,
               589998983, 589998984, 599939003, 599939004, 599998983,
               599998984, 599999999]
    for _ in range(100):
        rows.append(coord(rng.randrange(-180, 180), rng.choice(minutes),
                          rng.randrange(-90, 90), rng.choice(minutes)))
    path = Path(__file__).resolve().parent / 'carry_precision_all_valid.csv'
    with path.open('w', newline='') as f:
        writer = csv.writer(f); writer.writerow(['ID', 'COORD'])
        writer.writerows(enumerate(rows, 1))
    print(path.name, len(rows), 'rows')


if __name__ == '__main__':
    main()
