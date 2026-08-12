#!/usr/bin/env zsh

set -eu

if (( $# < 2 || $# > 3 )); then
  print -u2 "usage: $0 <intuigram-pid> <telegram-swift-pid> [seconds]"
  exit 2
fi

intuigram_pid=$1
telegram_pid=$2
seconds=${3:-30}

if (( seconds < 1 )); then
  print -u2 "seconds must be a positive integer"
  exit 2
fi

samples=$(mktemp -t intuigram-cpu.XXXXXX)
trap 'rm -f "$samples"' EXIT

print "second,intuigram_cpu,telegram_swift_cpu" > "$samples"
for second in {1..$seconds}; do
  intuigram_cpu=$(ps -p "$intuigram_pid" -o %cpu= | tr -d ' ')
  telegram_cpu=$(ps -p "$telegram_pid" -o %cpu= | tr -d ' ')
  if [[ -z "$intuigram_cpu" || -z "$telegram_cpu" ]]; then
    print -u2 "one benchmark process exited before sampling completed"
    exit 2
  fi
  print "$second,$intuigram_cpu,$telegram_cpu" >> "$samples"
  sleep 1
done

awk -F, '
  NR > 1 {
    intuigram += $2
    telegram += $3
    count += 1
  }
  END {
    printf "Intuigram mean CPU: %.2f%%\n", intuigram / count
    printf "Telegram Swift mean CPU: %.2f%%\n", telegram / count
    if (intuigram >= telegram) {
      print "FAIL: Intuigram did not stay below Telegram Swift"
      exit 1
    }
    print "PASS: Intuigram stayed below Telegram Swift"
  }
' "$samples"
