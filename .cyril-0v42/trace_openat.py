# gdb-python openat tracer (strace substitute; strace not installed, perf
# needs root). Prints every openat's pathname + whether O_TRUNC (0x200) is
# set, straight from the syscall-entry registers (x86-64: rsi=path, rdx=flags)
# — kernel-boundary ground truth independent of the probe's own reporting.
# Usage: gdb -batch -q -x trace_openat.py --args <binary> <argv...>
import gdb

gdb.execute("set pagination off")
gdb.execute("set confirm off")
gdb.execute("catch syscall openat")
gdb.execute("run", to_string=True)
while True:
    try:
        rdx = int(gdb.parse_and_eval("$rdx"))
        try:
            path = gdb.execute("x/s $rsi", to_string=True).split(":", 1)[1].strip()
        except gdb.error:
            path = "?"
        print("OPENAT %s trunc=%d" % (path, 1 if (rdx & 0x200) else 0))
        gdb.execute("continue", to_string=True)
    except gdb.error:
        break
