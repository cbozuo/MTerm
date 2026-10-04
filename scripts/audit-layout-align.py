"""Audit `alignment:` on HorizontalLayout in ui/*.slint.

Slint semantics:
  HorizontalLayout.alignment  -> main axis (horizontal)
  VerticalLayout.alignment    -> main axis (vertical)

Setting `alignment` on a HorizontalLayout takes the row off the stretch-driven
distribution: the layout stops handing leftover space to `horizontal-stretch`
children and aligns the whole row instead. Two shapes, one root cause:

  alignment: center  -> the row drifts to the middle. On a full-width row with
                        rows of differing content width you get a staircase
                        (each row starts at a different x).
  alignment: start   -> the row packs to the left and the trailing items stay
                        glued to the text instead of the right edge. Measured on
                        the storage path bar: inner right edge 738.5px, last
                        icon ends at 328.5px -- 410px of dead space.

Both are correct only for a self-contained row that is *meant* to be aligned
rather than stretched -- a fixed-width pill, or a row with no stretched child at
all. The test is therefore: does the layout have a DIRECT child carrying
`horizontal-stretch`? If yes, `alignment` is redundant at best and a bug at
worst. Nested child layouts' stretch belongs to them, not to this row, so only
direct children count.

Exit code 1 if any RISK line is printed, so it can gate a build.
"""
import re
import sys
import glob

ALIGN = re.compile(r'^\s*alignment:\s*(center|start|end)\s*;', re.M)


def direct_children(lines, start, end):
    """Yield (lineno, text) for lines that sit at depth 1 inside the block."""
    depth = 0
    for k in range(start, end + 1):
        before = depth
        depth += lines[k].count('{') - lines[k].count('}')
        # a line that opens a child block: depth goes 0 -> 1 right after it
        if before == 0 and depth == 1 and k > start:
            yield k + 1, lines[k]
        # a bare property line at depth 1
        if before == 1 and depth == 1 and '{' not in lines[k] and '}' not in lines[k]:
            yield k + 1, lines[k]


risks = 0
for path in sorted(glob.glob('ui/*.slint')):
    lines = open(path, encoding='utf-8').read().split('\n')
    i = 0
    while i < len(lines):
        line = lines[i]
        if not re.search(r'\bHorizontalLayout\b[^{]*\{', line):
            i += 1
            continue
        start = i
        depth = line.count('{') - line.count('}')
        j = i
        while depth > 0 and j + 1 < len(lines):
            j += 1
            depth += lines[j].count('{') - lines[j].count('}')
        body = '\n'.join(lines[start:j + 1])
        m = ALIGN.search(body)
        if m:
            kids = list(direct_children(lines, start, j))
            stretched = [t for _, t in kids
                         if re.search(r'^\s*horizontal-stretch\s*:', t)]
            if stretched:
                risks += 1
                print('RISK %s:%d  HorizontalLayout has alignment: %s '
                      'AND a direct child with horizontal-stretch'
                      % (path, start + 1, m.group(1)))
                for t in stretched:
                    print('       child: %s' % t.strip())
        i = j + 1

if not risks:
    print('clean: no HorizontalLayout combines alignment with a stretched child')
sys.exit(1 if risks else 0)
