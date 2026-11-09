# Debugging log

Method: **Observe → Localize → Hypothesize → Instrument → Modify → Test → Reproduce.**

## 1. Producer/consumer deadlock

* **Observe:** `prodcons` ended with `ERROR DEADLOCK: 4 task(s) blocked forever`.
  The suite then panicked: `tasks still hold the buffer`.
* **Localize:** the trace showed producers leaving with `why=block ran=0`
  after 4 items, then both consumers blocking too, with 0 wakeups.
* **Hypothesize:** 4 = buffer capacity, so `empty_slots` never gets permits back.
* **Modify:** the consumer was missing `empty_slots.signal()` after taking an item.
* **Instrument:** the scenario now reports a FAIL instead of panicking if a run deadlocks.
* **Test/Reproduce:** prodcons passes (50 items, sum conserved, max occupancy 4).
* **Lesson:** the kernel's deadlock detector (no READY task, no sleeper,
  tasks left) turned a silent hang into a precise diagnosis.

## 2. A wrong conclusion about exit codes (measurement error)

* **Observe:** exit statuses printed as 0 even when the log said `1 failed`.
* **Hypothesis 1 (wrong):** QEMU drops the test device's failure code. A
  message added after the test-device write never printed, which seemed to
  confirm that the write stops QEMU but the code is lost.
* **Localize:** commands were sent as `wsl -- bash -lc '... $? ...'`. WSL
  runs them through an outer shell, which expanded `$?` to `0` before the
  inner script ran. Re-measured from a script file: QEMU exits 1 for
  `exit(1)`, 0 for `exit(0)`, and `run.sh` returns 101 for a panic.
* **Side effect found:** the extra diagnostic print raced with QEMU's
  shutdown (`qemu_` appeared in one log and not the other), which broke the
  byte-identical repeatability check. It was removed.
* **Kept:** `run.sh` still derives its result from the kernel's own
  `SHUTDOWN failed=N` / `PANIC` line, which also distinguishes a panic (101)
  from a timeout (124) and a silent stop (125).
* **Lesson:** verify the measuring instrument before trusting a surprising measurement.

## 3. A check that tested the wrong thing

* `MEM.freelist_beats_bump` failed for 64-byte requests: all allocators got
  2099/2099. With uniform small sizes the live set always fits, so no
  allocator can win. The check now applies to mixed sizes only, and the
  uniform case checks that all three are identical.

## 4. Expected extra slice in TC1

* A task with a 20-tick burst and q=4 was dispatched 6 times, not 5. The
  trace shows the 6th slice with `ran=0`: the 5th slice expires on the same
  tick the burst completes, so the task is preempted before it notices it
  is done. The check allows `slices..=slices+1`, and the report explains
  the off-by-one.

## 5. Unsafe/raw-pointer warnings during bring-up

* Direct `fn as usize` casts and unneeded `unsafe` around `addr_of!` were
  flagged by rustc. Fixed with explicit function-pointer casts; the build
  is warning-free.

_Add your own entries here as you extend the kernel._
