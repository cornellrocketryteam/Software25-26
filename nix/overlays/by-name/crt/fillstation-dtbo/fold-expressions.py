#!/usr/bin/env python3
import re
import sys

if len(sys.argv) != 3:
    print(f"Usage: {sys.argv[0]} <input.dts> <output.dts>")
    sys.exit(1)

input_file = sys.argv[1]
output_file = sys.argv[2]

text = open(input_file, "r").read()

# Match: (0x0088), ((val_expr) + (3))
# where val_expr can be hex/dec constants joined by |
# e.g. (0x00010000|0) or just (0x00010000)
pat = re.compile(
    r"\(\s*(0x[0-9a-fA-F]+)\s*\)\s*,\s*"
    r"\(\s*\(\s*([\s0-9a-fA-Fx|]+)\s*\)\s*\+\s*\(\s*([0-9]+)\s*\)\s*\)"
)

def eval_or_expr(s):
    """Evaluate a |-separated expression of hex/dec constants."""
    parts = s.split('|')
    result = 0
    for p in parts:
        p = p.strip()
        result |= int(p, 0)  # int(x, 0) auto-detects hex/dec
    return result

def repl(m):
    off = int(m.group(1), 16)
    base = eval_or_expr(m.group(2))
    add = int(m.group(3), 10)
    return f"0x{off:04x} 0x{(base+add):08x}"

new_text, n = pat.subn(repl, text)
open(output_file, "w").write(new_text)
print(f"Wrote {output_file} with {n} replacements")
