#!/usr/local/bin/bash
coproc nc -l localhost 3000

while true; do
  echo 'hi'
  read -r -t 5 -p 'waiting for input' txt
  echo "${txt@U}"
done <&"${COPROC[0]}" >&"${COPROC[1]}"

kill "$COPROC_PID"
