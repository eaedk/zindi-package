#!/usr/bin/env bash
# Print your rank and the top 3 every 10 minutes.
# Usage: ./leaderboard_watch.sh Crop-Yield-Forecast-Challenge   (needs jq)
set -euo pipefail
competition="${1:?usage: $0 <competition-id>}"
while true; do
  printf '%s  rank: %s\n' "$(date +%H:%M)" "$(zindi rank "$competition")"
  zindi leaderboard "$competition" -n 3 --json | jq -r '.[] | "  #\(.rank)  \(.name)  \(.score)"'
  sleep 600
done
