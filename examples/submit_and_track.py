"""Submit a file at the end of a training run and record the score.

    python submit_and_track.py submission.csv --competition Crop-Yield-Forecast-Challenge \
        --comment "lgbm, 5 folds"

Credentials come from ZINDI_TOKEN (or ZINDI_USERNAME/ZINDI_PASSWORD, or `zindi login`).
"""

from __future__ import annotations

import argparse
import csv
from datetime import datetime, timezone
from pathlib import Path

import zindi


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("file", type=Path)
    parser.add_argument("--competition", "-c", default=None)
    parser.add_argument("--comment", "-m", default="")
    parser.add_argument("--log", type=Path, default=Path("scores.csv"))
    args = parser.parse_args()

    with zindi.Client(competition=args.competition) as client:
        if client.limits().remaining_today == 0:
            raise SystemExit("No submission left today; try again tomorrow.")
        sub = client.submit(args.file, comment=args.comment, wait=True)
        rank = client.rank()

    if sub.is_failed:
        raise SystemExit(f"Submission failed: {sub.status_description}")

    new_file = not args.log.exists()
    with args.log.open("a", newline="", encoding="utf-8") as fh:
        writer = csv.writer(fh)
        if new_file:
            writer.writerow(["time", "submission", "comment", "public_score", "rank"])
        writer.writerow(
            [datetime.now(timezone.utc).isoformat(timespec="seconds"), sub.id, args.comment, sub.public_score, rank]
        )
    print(f"score={sub.public_score} rank={rank} (logged to {args.log})")


if __name__ == "__main__":
    main()
