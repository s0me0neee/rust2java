#!/bin/zsh
# Corpus check: each tests/corpus/NAME.rs through rustc and both transpiler modes, one at a time.
#   tests/corpus/run.sh [name ...]     (build first: cargo build --release)
# Outputs go to target/corpus/<rust|exact|simple>/; the summary lists PASS / OUTPUT / EXIT /
# JAVAC / TRANSPILE per program and mode. Simple mode compares only whether the exit is zero.
repo=${0:A:h:h:h}
src=$repo/tests/corpus
out=$repo/target/corpus
bin=$repo/target/release/rust2java
mkdir -p $out/rust $out/exact $out/simple
names=(${@:-${(f)"$(ls $src/*.rs | xargs -n1 basename | sed 's/\.rs$//')"}})
summary=$out/summary.txt
: > $summary
for n in $names; do
  in=/dev/null; [[ -f $src/$n.stdin ]] && in=$src/$n.stdin
  if [[ ! -f $out/rust/$n.out || $src/$n.rs -nt $out/rust/$n.out ]]; then
    if ! rustc --edition 2021 -O -o $out/rust/$n $src/$n.rs 2>$out/rust/$n.err; then
      echo "$n RUSTC" >> $summary; continue
    fi
    $out/rust/$n < $in > $out/rust/$n.out 2>/dev/null; echo $? > $out/rust/$n.code
  fi
  want_code=$(cat $out/rust/$n.code)
  for mode in exact simple; do
    d=$out/$mode
    flag=(); [[ $mode == simple ]] && flag=(-s)
    rm -rf $d/$n.cls
    if ! $bin -q $flag $src/$n.rs -o $d/$n.java 2>$d/$n.err; then
      echo "$n $mode TRANSPILE" >> $summary; continue
    fi
    if ! javac -d $d/$n.cls $d/$n.java 2>>$d/$n.err; then
      echo "$n $mode JAVAC" >> $summary; continue
    fi
    java -cp $d/$n.cls Main < $in > $d/$n.out 2>>$d/$n.err; code=$?
    ok_code=1
    if [[ $mode == exact ]]; then
      [[ $code == $want_code ]] || ok_code=0
    else
      (( (code == 0) == (want_code == 0) )) || ok_code=0
    fi
    if (( ! ok_code )); then
      echo "$n $mode EXIT($code want $want_code)" >> $summary
    elif ! cmp -s $d/$n.out $out/rust/$n.out; then
      echo "$n $mode OUTPUT" >> $summary
    else
      echo "$n $mode PASS" >> $summary
    fi
  done
done
awk '{print $2, $3}' $summary | sed 's/(.*//' | sort | uniq -c
grep -v PASS $summary
