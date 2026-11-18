#!/bin/sh
# Size of the Java generated for every test case and probe; extra args are regexes whose matches get counted.
# Usage: tasks/size.sh ['Collectors\.toCollection' ...]   (prints the output dir last, for inspection)
set -e
cd "$(dirname "$0")/.."
out=$(mktemp -d)
cargo build -q 2>/dev/null
./target/debug/rust2java tests/cases/*.rs -o "$out/cases" >/dev/null 2>&1 || true
./target/debug/rust2java tasks/probes/*.rs -o "$out/probes" >/dev/null 2>&1 || true
printf 'bytes %s\n' "$(cat "$out"/cases/*.java "$out"/probes/*.java | wc -c | tr -d ' ')"
for pat in "$@"; do
    printf '%6s  %s\n' "$(cat "$out"/cases/*.java "$out"/probes/*.java | rg -o -- "$pat" | wc -l | tr -d ' ')" "$pat"
done
echo "$out"
