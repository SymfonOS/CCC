"""Level 6: exact short building cycles with OR-Tools CP-SAT.

For a pattern, find configurations C_1..C_L (emitter ring positions + active flags)
connected by valid single-line transitions such that
  * no forbidden cell (neighbour of the pattern, not in it) ever gets exactly 2 hits,
  * no pattern cell ever gets >= 3 hits,
  * every pattern cell gets exactly 2 hits in at least one tick.
Then the cycle can be traversed forwards and backwards in any order; each traversal
yields one copy (cost L lines per copy after the setup).

Usage: python3 cycles.py <in-file> <out-file-existing> <out-file-result> [time_limit] [Lmax]
"""
import sys
from ortools.sat.python import cp_model

DIRS = [(0, 2), (1, 1), (1, -1), (0, -2), (-1, -1), (-1, 1)]
NB = [(0, 2), (0, -2), (1, 1), (1, -1), (-1, 1), (-1, -1)]


def board(r):
    g = []
    for k in range(2 * r + 1):
        m = min(k, 2 * r - k)
        row = ['-'] * (4 * r + 1)
        for i in range(r + 1 + m):
            edge = k in (0, 2 * r) or i == 0 or i == r + m
            row[r - m + 2 * i] = '*' if edge else 'O'
        g.append(row)
    return g


def ring(r):
    return ([(0, r + 2 * i) for i in range(r)] + [(i, 3 * r + i) for i in range(r)]
            + [(r + i, 4 * r - i) for i in range(r)] + [(2 * r, 3 * r - 2 * i) for i in range(r)]
            + [(2 * r - i, r - i) for i in range(r)] + [(r - i, i) for i in range(r)])


def home(r):
    return [5 * r, 0, r, 2 * r, 3 * r, 4 * r]


def rays(r):
    g = board(r)
    H, W = len(g), len(g[0])
    rg = ring(r)
    out = []
    for e in range(6):
        per = []
        for p in range(len(rg)):
            y, x = rg[p]
            cells = []
            while True:
                y += DIRS[e][0]
                x += DIRS[e][1]
                if not (0 <= y < H and 0 <= x < W) or g[y][x] != 'O':
                    break
                cells.append((y, x))
            per.append(cells)
        out.append(per)
    return out


def simulate(r, pat, prog, limit):
    """Ticks (0-based) of submissions, stops after `limit`."""
    g = board(r)
    H, W = len(g), len(g[0])
    rg = ring(r)
    n = len(rg)
    cells = [(y, x) for y in range(H) for x in range(W) if pat[y][x] == 'M']
    pset = set(cells)
    idx = home(r)
    act = [False] * 6
    subs = []
    for t, line in enumerate(prog):
        for e, c in enumerate(line):
            if c == 'F':
                idx[e] = (idx[e] + 1) % n
            elif c == 'B':
                idx[e] = (idx[e] - 1) % n
            elif c == 'T':
                act[e] = not act[e]
        hits = {}
        for e in range(6):
            if not act[e]:
                continue
            y, x = rg[idx[e]]
            dy, dx = DIRS[e]
            while True:
                y += dy
                x += dx
                if not (0 <= y < H and 0 <= x < W) or g[y][x] not in 'OM':
                    break
                hits[(y, x)] = hits.get((y, x), 0) + 1
        for (y, x), h in hits.items():
            if h == 2:
                g[y][x] = 'M'
            elif h >= 3:
                g[y][x] = 'O'
        if cells and all(g[y][x] == 'M' for y, x in cells):
            ok = all(not (0 <= y + dy < H and 0 <= x + dx < W) or g[y + dy][x + dx] != 'M'
                     or (y + dy, x + dx) in pset for y, x in cells for dy, dx in NB)
            if ok:
                for y, x in cells:
                    g[y][x] = 'O'
                subs.append(t)
                if len(subs) >= limit:
                    break
    return subs


def ringdist(a, b, n):
    d = abs(a - b) % n
    return min(d, n - d)


def solve_cycle(r, pat, L, time_limit):
    """Returns (setup distance, configs) or None. configs[t][e] = (pos, active)."""
    g = board(r)
    H, W = len(g), len(g[0])
    n = 6 * r
    ry = rays(r)
    P = [(y, x) for y in range(H) for x in range(W) if pat[y][x] == 'M']
    pset = set(P)
    F = sorted({(y + dy, x + dx) for y, x in P for dy, dx in NB
                if 0 <= y + dy < H and 0 <= x + dx < W and g[y + dy][x + dx] == 'O'} - pset)
    # ring position of emitter e whose beam passes through a cell
    origin = [dict() for _ in range(6)]
    for e in range(6):
        for p in range(n):
            for c in ry[e][p]:
                origin[e][c] = p
    # allowed positions: useful positions within L-1 steps of a position hitting the pattern
    allowed = []
    for e in range(6):
        hitp = {origin[e][c] for c in P if c in origin[e]}
        useful = {p for p in range(n) if ry[e][p]}
        allowed.append(sorted({(p + d) % n for p in hitp for d in range(-(L - 1), L)} & useful))
    m = cp_model.CpModel()
    z = [[{p: m.NewBoolVar('') for p in allowed[e]} for e in range(6)] for _ in range(L)]
    a = [[m.NewBoolVar('') for _ in range(6)] for _ in range(L)]
    on = [[{} for _ in range(6)] for _ in range(L)]
    for t in range(L):
        for e in range(6):
            m.AddExactlyOne(z[t][e].values())
            for p, v in z[t][e].items():
                o = m.NewBoolVar('')
                m.AddBoolAnd([v, a[t][e]]).OnlyEnforceIf(o)
                m.AddBoolOr([v.Not(), a[t][e].Not()]).OnlyEnforceIf(o.Not())
                on[t][e][p] = o
    # transitions: every emitter moves at most one ring step; a toggle forbids moving
    for t in range(L - 1):
        for e in range(6):
            tog = m.NewBoolVar('')
            m.AddBoolXOr([a[t][e], a[t + 1][e], tog.Not()])
            for q, v in z[t + 1][e].items():
                prev = [z[t][e][p] for p in allowed[e] if ringdist(p, q, n) <= 1]
                m.AddBoolOr(prev).OnlyEnforceIf(v)
            for p, v in z[t][e].items():
                if p in z[t + 1][e]:
                    m.AddImplication(v, z[t + 1][e][p]).OnlyEnforceIf(tog)
                else:
                    m.AddBoolOr([v.Not(), tog.Not()])

    def count(t, c):
        return sum(on[t][e][origin[e][c]] for e in range(6) if c in origin[e] and origin[e][c] in on[t][e])

    for t in range(L):
        for c in F:
            m.Add(count(t, c) != 2)
    for c in P:
        gs = []
        for t in range(L):
            cnt = count(t, c)
            m.Add(cnt <= 2)
            gv = m.NewBoolVar('')
            m.Add(cnt == 2).OnlyEnforceIf(gv)
            gs.append(gv)
        m.AddBoolOr(gs)
    # secondary objective: setup distance from the start corners
    h = home(r)
    d = m.NewIntVar(0, n, 'd')
    for e in range(6):
        used = m.NewBoolVar('')
        m.AddBoolOr([a[t][e] for t in range(L)]).OnlyEnforceIf(used)
        m.AddBoolAnd([a[t][e].Not() for t in range(L)]).OnlyEnforceIf(used.Not())
        for p, v in z[0][e].items():
            m.Add(d >= ringdist(h[e], p, n)).OnlyEnforceIf([v, used])
    m.Minimize(d)
    s = cp_model.CpSolver()
    s.parameters.max_time_in_seconds = time_limit
    s.parameters.num_workers = 4
    st = s.Solve(m)
    if st not in (cp_model.OPTIMAL, cp_model.FEASIBLE):
        return None
    cfgs = []
    for t in range(L):
        row = []
        for e in range(6):
            p = next(p for p, v in z[t][e].items() if s.Value(v))
            row.append((p, bool(s.Value(a[t][e]))))
        cfgs.append(row)
    return s.Value(d), cfgs


def build_program(r, cfgs, k, pat):
    """Setup (inactive), activation line = C_1, cycle lines, then alternate
    backward / forward traversals (each starting with an idle line)."""
    n = 6 * r
    L = len(cfgs)
    h = home(r)
    used = [any(cfgs[t][e][1] for t in range(L)) for e in range(6)]
    moves = []
    for e in range(6):
        if not used[e]:
            moves.append([])
            continue
        fwd = (cfgs[0][e][0] - h[e]) % n
        moves.append(['F'] * fwd if fwd <= n - fwd else ['B'] * (n - fwd))
    dmax = max(len(mv) for mv in moves)
    prog = [[moves[e][i] if i < len(moves[e]) else '-' for e in range(6)] for i in range(dmax)]
    prog.append(['T' if cfgs[0][e][1] else '-' for e in range(6)])

    def trans(c0, c1):
        line = []
        for e in range(6):
            if not used[e]:
                line.append('-')
            elif c0[e][1] != c1[e][1]:
                line.append('T')
            elif (c0[e][0] + 1) % n == c1[e][0]:
                line.append('F')
            elif (c1[e][0] + 1) % n == c0[e][0]:
                line.append('B')
            else:
                line.append('-')
        return line

    fwd = [trans(cfgs[t], cfgs[t + 1]) for t in range(L - 1)]
    inv = {'F': 'B', 'B': 'F', 'T': 'T', '-': '-'}
    rev = [[inv[c] for c in line] for line in reversed(fwd)]
    prog += fwd
    forward = False
    for _ in range(k + 1):
        prog.append(['-'] * 6)
        prog += fwd if forward else rev
        forward = not forward
    subs = simulate(r, pat, prog, k)
    if len(subs) < k:
        return None
    return prog[:subs[k - 1] + 1]


def main():
    inp, old_out, res_out = sys.argv[1], sys.argv[2], sys.argv[3]
    tl = float(sys.argv[4]) if len(sys.argv) > 4 else 20
    lmax = int(sys.argv[5]) if len(sys.argv) > 5 else 4
    t = open(inp).read().split()
    nprog = int(t[0])
    pos = 1
    olds = [[l.split() for l in b.split('\n')] for b in open(old_out).read().strip('\n').split('\n\n')]
    result = []
    stats = {}
    before = after = 0
    for i in range(nprog):
        r, rows, cols, k = map(int, t[pos:pos + 4])
        pos += 4
        pat = t[pos:pos + rows]
        pos += rows
        old = olds[i]
        assert len(simulate(r, pat, old, k)) == k
        best = old
        for L in range(1, lmax + 1):
            if L * k >= len(best):
                break
            sol = solve_cycle(r, pat, L, tl)
            if sol is None:
                continue
            prog = build_program(r, sol[1], k, pat)
            if prog is not None and len(prog) < len(best):
                assert len(simulate(r, pat, prog, k)) == k
                best = prog
                stats[L] = stats.get(L, 0) + 1
            break
        before += len(old)
        after += len(best)
        result.append(best)
        print(f"{i}: k={k} alt={len(old)} neu={len(best)}", flush=True)
    with open(res_out, 'w') as f:
        for p in result:
            for line in p:
                f.write(' '.join(line) + '\n')
            f.write('\n')
    print(f"vorher {before} nachher {after} verbessert je L: {stats}")


if __name__ == '__main__':
    main()
