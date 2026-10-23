#!/usr/bin/env python3
"""Relaxed cyclic cycle search (CP-SAT) for Level 6.

Looks for a closed cycle of L emitter configurations C_0..C_{L-1} such that, starting from an
empty board (pattern + halo cells empty, which is exactly the state right after a submission),
the pattern is submitted at tick L-1 and never earlier. Between ticks anything is allowed:
pattern cells may be destroyed and rebuilt, halo cells may hold matter temporarily.
Program = setup (inactive moves) + activation line (= C_0) + L-1 lines, then (K-1) * L lines.

Usage: python3 cyclic.py <in> <out-old> <out-new> [time_limit_per_L] [lmax] [procs]
"""
import os
import sys
from multiprocessing import Pool

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from cycles import NB, board, home, rays, ringdist, simulate  # noqa: E402
from ortools.sat.python import cp_model  # noqa: E402


def relevant(r, pat):
    g = board(r)
    H, W = len(g), len(g[0])
    P = [(y, x) for y in range(H) for x in range(W) if pat[y][x] == 'M']
    pset = set(P)
    F = sorted({(y + dy, x + dx) for y, x in P for dy, dx in NB
                if 0 <= y + dy < H and 0 <= x + dx < W and g[y + dy][x + dx] == 'O'} - pset)
    return P, F


def solve_cyclic(r, pat, L, time_limit, workers=2):
    """Returns (setup distance, cfgs) or None; cfgs[t][e] = (pos, active)."""
    n = 6 * r
    ry = rays(r)
    P, F = relevant(r, pat)
    origin = [dict() for _ in range(6)]
    for e in range(6):
        for p in range(n):
            for c in ry[e][p]:
                origin[e][c] = p
    allowed = []
    for e in range(6):
        hitp = {origin[e][c] for c in P if c in origin[e]}
        useful = {p for p in range(n) if ry[e][p]}
        allowed.append(sorted({(p + d) % n for p in hitp for d in range(-L, L + 1)} & useful))
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
    # cyclic transitions (including L-1 -> 0): move at most one step, or toggle without moving
    for t in range(L):
        t1 = (t + 1) % L
        if L == 1:
            break
        for e in range(6):
            tog = m.NewBoolVar('')
            m.AddBoolXOr([a[t][e], a[t1][e], tog.Not()])
            for q, v in z[t1][e].items():
                prev = [z[t][e][p] for p in allowed[e] if ringdist(p, q, n) <= 1]
                m.AddBoolOr(prev).OnlyEnforceIf(v)
            for p, v in z[t][e].items():
                m.AddImplication(v, z[t1][e][p]).OnlyEnforceIf(tog)
    if L == 1:
        # single config: must be static (no transition needed), nothing to add
        pass

    cells = P + F
    npat = len(P)

    def count(t, c):
        return sum(on[t][e][origin[e][c]] for e in range(6) if c in origin[e] and origin[e][c] in on[t][e])

    # matter state per tick, start state (before tick 0) = all relevant cells empty
    mat = [[m.NewBoolVar('') for _ in cells] for _ in range(L)]
    for t in range(L):
        for ci, c in enumerate(cells):
            cnt = count(t, c)
            two = m.NewBoolVar('')
            three = m.NewBoolVar('')
            m.Add(cnt == 2).OnlyEnforceIf(two)
            m.Add(cnt != 2).OnlyEnforceIf(two.Not())
            m.Add(cnt >= 3).OnlyEnforceIf(three)
            m.Add(cnt <= 2).OnlyEnforceIf(three.Not())
            if t == 0:
                # previous state empty: matter iff exactly two hits
                m.Add(mat[t][ci] == two)
            else:
                keep = m.NewBoolVar('')
                prev = mat[t - 1][ci]
                m.AddBoolAnd([prev, three.Not()]).OnlyEnforceIf(keep)
                m.AddBoolOr([prev.Not(), three]).OnlyEnforceIf(keep.Not())
                m.AddBoolOr([two, keep]).OnlyEnforceIf(mat[t][ci])
                m.AddBoolAnd([two.Not(), keep.Not()]).OnlyEnforceIf(mat[t][ci].Not())
    # submission exactly at tick L-1, not earlier
    for ci in range(len(cells)):
        if ci < npat:
            m.Add(mat[L - 1][ci] == 1)
        else:
            m.Add(mat[L - 1][ci] == 0)
    for t in range(L - 1):
        m.AddBoolOr([mat[t][ci].Not() for ci in range(npat)] + [mat[t][ci] for ci in range(npat, len(cells))])
    # secondary objective: setup distance
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
    s.parameters.num_workers = workers
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

    cyc = [trans(cfgs[t], cfgs[(t + 1) % L]) for t in range(L)]  # cyc[L-1] = C_{L-1} -> C_0
    prog += cyc[:L - 1]
    for _ in range(k - 1):
        prog += [cyc[L - 1]] + cyc[:L - 1]
    subs = simulate(r, pat, prog, k)
    if len(subs) < k:
        return None
    return prog[:subs[k - 1] + 1]


def period_of(r, pat, prog, k):
    subs = simulate(r, pat, prog, k)
    if len(subs) < 2:
        return len(prog)
    return subs[-1] - subs[-2]


def work(args):
    i, r, pat, k, old, tl, lmax = args
    best = old
    pcur = period_of(r, pat, old, k)
    info = ''
    for L in range(1, min(lmax, pcur - 1) + 1):
        if L * k >= len(best):
            break
        sol = solve_cyclic(r, pat, L, tl)
        if sol is None:
            continue
        prog = build_program(r, sol[1], k, pat)
        if prog is not None and len(prog) < len(best):
            assert len(simulate(r, pat, prog, k)) == k
            best = prog
            info = f' L={L} (vorher Periode {pcur})'
        break
    return i, best, info


def main():
    inp, old_out, res_out = sys.argv[1], sys.argv[2], sys.argv[3]
    tl = float(sys.argv[4]) if len(sys.argv) > 4 else 20
    lmax = int(sys.argv[5]) if len(sys.argv) > 5 else 6
    procs = int(sys.argv[6]) if len(sys.argv) > 6 else 2
    t = open(inp).read().split()
    nprog = int(t[0])
    pos = 1
    olds = [[l.split() for l in b.split('\n')] for b in open(old_out).read().strip('\n').split('\n\n')]
    jobs = []
    for i in range(nprog):
        r, rows, cols, k = map(int, t[pos:pos + 4])
        pos += 4
        pat = t[pos:pos + rows]
        pos += rows
        jobs.append((i, r, pat, k, olds[i], tl, lmax))
    # most valuable first: large K
    order = sorted(range(nprog), key=lambda i: -jobs[i][3])
    result = list(olds)

    def dump():
        with open(res_out, 'w') as f:
            for p in result:
                for line in p:
                    f.write(' '.join(line) + '\n')
                f.write('\n')

    before = sum(len(p) for p in olds)
    done = 0
    with Pool(procs) as pool:
        for i, best, info in pool.imap_unordered(work, [jobs[i] for i in order]):
            result[i] = best
            done += 1
            if info:
                print(f"{i}: k={jobs[i][3]} alt={len(olds[i])} neu={len(best)}{info}", flush=True)
            if done % 10 == 0:
                dump()
    dump()
    print(f"vorher {before} nachher {sum(len(p) for p in result)}")


if __name__ == '__main__':
    main()
