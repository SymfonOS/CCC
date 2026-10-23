"""Level 6: exakte Verkürzung einzelner Programme per SAT (Bounded Model Checking).

Für ein Muster wird gefragt: gibt es ein Programm mit T Zeilen, das genau k Kopien abgibt?
T wird ausgehend von der bisherigen Länge so lange verringert, bis UNSAT oder Zeitlimit.
Aufruf: python3 sat_opt.py <in-datei> <out-datei> [zeitlimit_s] [index,...]
"""
import sys, time, threading
from pysat.solvers import Cadical153
from pysat.card import CardEnc, EncType
from pysat.formula import IDPool

DIRS = [(0, 2), (1, 1), (1, -1), (0, -2), (-1, -1), (-1, 1)]
NEIGH = [(0, 2), (0, -2), (1, 1), (1, -1), (-1, 1), (-1, -1)]


def empty_board(r):
    rows = []
    for k in range(2 * r + 1):
        m = min(k, 2 * r - k)
        row = ['-'] * (4 * r + 1)
        for i in range(r + 1 + m):
            edge = k == 0 or k == 2 * r or i == 0 or i == r + m
            row[r - m + 2 * i] = '*' if edge else 'O'
        rows.append(row)
    return rows


def ring(r):
    v = []
    v += [(0, r + 2 * i) for i in range(r)]
    v += [(i, 3 * r + i) for i in range(r)]
    v += [(r + i, 4 * r - i) for i in range(r)]
    v += [(2 * r, 3 * r - 2 * i) for i in range(r)]
    v += [(2 * r - i, r - i) for i in range(r)]
    v += [(r - i, i) for i in range(r)]
    return v


def home(r):
    return [5 * r, 0, r, 2 * r, 3 * r, 4 * r]


def get(g, y, x):
    if y < 0 or x < 0 or y >= len(g) or x >= len(g[0]):
        return ''
    return g[y][x]


def geometry(r, pattern):
    board = empty_board(r)
    rg = ring(r)
    n = len(rg)
    ray = [[[] for _ in range(n)] for _ in range(6)]
    for e in range(6):
        for p in range(n):
            y, x = rg[p]
            while True:
                y += DIRS[e][0]
                x += DIRS[e][1]
                if get(board, y, x) != 'O':
                    break
                ray[e][p].append((y, x))
    pat = [(y, x) for y, row in enumerate(pattern) for x, c in enumerate(row) if c == 'M']
    ps = set(pat)
    forb = set()
    for (y, x) in pat:
        for dy, dx in NEIGH:
            if get(board, y + dy, x + dx) == 'O' and (y + dy, x + dx) not in ps:
                forb.add((y + dy, x + dx))
    return n, ray, pat, sorted(forb)


def simulate(r, pattern, prog):
    n, ray, pat, forb = geometry(r, pattern)
    M = set()
    idx = home(r)
    act = [False] * 6
    cnt = 0
    for line in prog:
        for e in range(6):
            c = line[e]
            if c == 'F':
                idx[e] = (idx[e] + 1) % n
            elif c == 'B':
                idx[e] = (idx[e] - 1) % n
            elif c == 'T':
                act[e] = not act[e]
        hits = {}
        for e in range(6):
            if act[e]:
                for c in ray[e][idx[e]]:
                    hits[c] = hits.get(c, 0) + 1
        for c, h in hits.items():
            if h == 2:
                M.add(c)
            elif h >= 3:
                M.discard(c)
        if all(c in M for c in pat) and not any(c in M for c in forb):
            for c in pat:
                M.discard(c)
            cnt += 1
    return cnt


def build(r, pattern, T, free_start=False):
    n, ray, pat, forb = geometry(r, pattern)
    origin = [{} for _ in range(6)]
    for e in range(6):
        for p in range(n):
            for c in ray[e][p]:
                origin[e][c] = p
    pool = IDPool()
    V = pool.id
    cl = []
    h = home(r)
    P = lambda t, e, p: V(('P', t, e, p))
    A = lambda t, e: V(('A', t, e))
    # t = 0: home, inactive
    for e in range(6):
        if free_start:
            lits = [P(0, e, p) for p in range(n)]
            cl.append(lits)
            cl.extend(CardEnc.atmost(lits, 1, vpool=pool, encoding=EncType.seqcounter).clauses)
            continue
        for p in range(n):
            cl.append([P(0, e, p)] if p == h[e] else [-P(0, e, p)])
        cl.append([-A(0, e)])
    for t in range(1, T + 1):
        for e in range(6):
            lits = [P(t, e, p) for p in range(n)]
            cl.append(lits)
            cl.extend(CardEnc.atmost(lits, 1, vpool=pool, encoding=EncType.seqcounter).clauses)
            for p in range(n):
                a, b, c = P(t, e, (p - 1) % n), P(t, e, p), P(t, e, (p + 1) % n)
                cl.append([-P(t - 1, e, p), a, b, c])
                # Bewegung und Umschalten schließen sich aus
                cl.append([-P(t - 1, e, p), b, A(t, e), -A(t - 1, e)])
                cl.append([-P(t - 1, e, p), b, -A(t, e), A(t - 1, e)])
    X = lambda t, e, p: V(('X', t, e, p))

    def xvar(t, e, p):
        x = X(t, e, p)
        if ('xd', t, e, p) not in done:
            done.add(('xd', t, e, p))
            cl.extend([[-x, P(t, e, p)], [-x, A(t, e)], [x, -P(t, e, p), -A(t, e)]])
        return x
    done = set()

    def totalizer(lits):
        # liefert unäre Ausgänge [>=1, >=2, >=3] (Äquivalenz)
        if len(lits) == 1:
            return [lits[0]]
        m = len(lits) // 2
        a, b = totalizer(lits[:m]), totalizer(lits[m:])
        size = min(3, len(a) + len(b))
        out = [pool.id() for _ in range(size)]
        for i in range(len(a) + 1):
            for j in range(len(b) + 1):
                s = i + j
                if 1 <= s <= size:
                    c = [out[s - 1]]
                    if i:
                        c.append(-a[i - 1])
                    if j:
                        c.append(-b[j - 1])
                    cl.append(c)
                if s < size:
                    c = [-out[s]]
                    if i < len(a):
                        c.append(a[i])
                    if j < len(b):
                        c.append(b[j])
                    cl.append(c)
        return out

    cells = pat + forb
    isp = {c: True for c in pat}
    Mv = lambda t, c: V(('M', t, c))
    S = lambda t: V(('S', t))
    for c in cells:
        cl.append([-Mv(0, c)])
    cl.append([-S(0)])
    for t in range(1, T + 1):
        for c in cells:
            hl = [xvar(t, e, origin[e][c]) for e in range(6) if c in origin[e]]
            m = Mv(t, c)
            prev = Mv(t - 1, c)
            if len(hl) < 2:
                g2, g3 = None, None
            else:
                o = totalizer(hl)
                g2 = o[1]
                g3 = o[2] if len(o) > 2 else None
            if g2 is None:
                # nie genau 2 Treffer: Zustand bleibt (bis auf Abgabe)
                cl.append([-prev, m] + ([S(t - 1)] if c in isp else []))
                cl.append([prev, -m])
                if c in isp:
                    cl.append([-S(t - 1), -m])
                continue
            cl.append([-g2, m] + ([g3] if g3 else []))
            if g3:
                cl.append([-g3, -m])
            cl.append([g2, -prev, m] + ([S(t - 1)] if c in isp else []))
            cl.append([g2, prev, -m])
            if c in isp:
                cl.append([g2, -S(t - 1), -m])
        cond = [Mv(t, c) for c in pat] + [-Mv(t, c) for c in forb]
        for l in cond:
            cl.append([-S(t), l])
        cl.append([S(t)] + [-l for l in cond])
    return cl, pool, P, A, S, n


def bmc(r, pattern, k, T):
    cl, pool, P, A, S, n = build(r, pattern, T)
    svars = [S(t) for t in range(1, T + 1)]
    cl.extend(CardEnc.equals(svars, k, vpool=pool, encoding=EncType.seqcounter).clauses)
    cl.append([S(T)])
    return cl, P, A, n


def pure_cycle(r, pattern, L, m):
    cl, pool, P, A, S, n = build(r, pattern, L, free_start=True)
    cyc = [S(t) for t in range(1, L + 1)]
    cl.extend(CardEnc.equals(cyc, m, vpool=pool, encoding=EncType.seqcounter).clauses)
    cl.append([S(L)])
    for e in range(6):
        cl.append([-A(0, e), A(L, e)])
        cl.append([A(0, e), -A(L, e)])
        for p in range(n):
            cl.append([-P(0, e, p), P(L, e, p)])
    return cl, P, A, n


def cyclic(r, pattern, Tp, L, m):
    """Präfix (Tp Zeilen, 1 Kopie, endet mit Abgabe) + Zyklus (L Zeilen, m Kopien, endet mit Abgabe
    in derselben Emitter-Konfiguration wie am Präfixende)."""
    T = Tp + L
    cl, pool, P, A, S, n = build(r, pattern, T)
    pre = [S(t) for t in range(1, Tp + 1)]
    cl.extend(CardEnc.equals(pre, 1, vpool=pool, encoding=EncType.seqcounter).clauses)
    cl.append([S(Tp)])
    cyc = [S(t) for t in range(Tp + 1, T + 1)]
    cl.extend(CardEnc.equals(cyc, m, vpool=pool, encoding=EncType.seqcounter).clauses)
    cl.append([S(T)])
    for e in range(6):
        cl.append([-A(Tp, e), A(T, e)])
        cl.append([A(Tp, e), -A(T, e)])
        for p in range(n):
            cl.append([-P(Tp, e, p), P(T, e, p)])
    return cl, P, A, n


def decode(model, P, A, n, T):
    ms = set(l for l in model if l > 0)
    prog = []
    pos = [None] * 6
    act = [False] * 6
    for t in range(T + 1):
        line = []
        for e in range(6):
            p = next(p for p in range(n) if P(t, e, p) in ms)
            a = A(t, e) in ms
            if t > 0:
                if a != act[e]:
                    line.append('T')
                elif p == (pos[e] + 1) % n:
                    line.append('F')
                elif p == (pos[e] - 1) % n:
                    line.append('B')
                else:
                    line.append('-')
            pos[e], act[e] = p, a
        if t > 0:
            prog.append(line)
    return prog


def run(cl, P, A, n, T, budget):
    s = Cadical153(bootstrap_with=cl)
    timer = threading.Timer(budget, s.interrupt)
    timer.start()
    res = s.solve_limited(expect_interrupt=True)
    timer.cancel()
    if res is None:
        s.delete()
        return None, None
    out = decode(s.get_model(), P, A, n, T) if res else None
    s.delete()
    return res, out


def solve_T(r, pattern, k, T, budget):
    cl, P, A, n = bmc(r, pattern, k, T)
    return run(cl, P, A, n, T, budget)


def cycle_search(r, pattern, k, best, budget):
    """Beste Kombination Präfix + wiederholter Zyklus; gibt Programm oder None zurück."""
    found = None
    for m in (1, 2):
        for L in range(1, 12 * m):
            q, rem = divmod(k - 1, m)
            if q == 0:
                continue
            if q * L >= best:
                break
            res, _ = run(*pure_cycle(r, pattern, L, m), L, budget)
            print(f'   pure m={m} L={L}: {res}', flush=True)
            if not res:
                continue
            for Tp in range(1, 40):
                if Tp + q * L + (L if rem else 0) >= best:
                    break
                res, prog = run(*cyclic(r, pattern, Tp, L, m), Tp + L, budget)
                if res is None:
                    break
                if not res:
                    continue
                pre, cyc = prog[:Tp], prog[Tp:]
                full = pre + cyc * q
                if rem:
                    # Teilzyklus bis zur rem-ten Abgabe
                    for j in range(1, L + 1):
                        if simulate(r, pattern, full + cyc[:j]) == k:
                            full = full + cyc[:j]
                            break
                print(f'   m={m} L={L} Tp={Tp} -> {len(full)}', flush=True)
                if simulate(r, pattern, full) == k and len(full) < best:
                    best, found = len(full), full
                break
    return found


def parse(text):
    t = text.split()
    i = 0
    N = int(t[i]); i += 1
    tasks = []
    for _ in range(N):
        r, rows, _c, k = map(int, t[i:i + 4]); i += 4
        pattern = t[i:i + rows]; i += rows
        tasks.append((r, pattern, k))
    return tasks


def parse_out(text):
    return [[l.split() for l in b.split('\n') if l.strip()] for b in text.strip('\n').split('\n\n')]


def write_out(path, progs):
    with open(path, 'w') as f:
        for p in progs:
            for l in p:
                f.write(' '.join(l) + '\n')
            f.write('\n')


MAXBMC = 60


def main():
    inp, outp = sys.argv[1], sys.argv[2]
    budget = float(sys.argv[3]) if len(sys.argv) > 3 else 60
    tasks = parse(open(inp).read())
    progs = parse_out(open(outp).read())
    sel = [int(x) for x in sys.argv[4].split(',')] if len(sys.argv) > 4 else range(len(tasks))
    lock = threading.Lock()
    for i in sel:
        r, pattern, k = tasks[i]
        cur = progs[i]
        t0 = time.time()
        res = None
        if k >= 3:
            c = cycle_search(r, pattern, k, len(cur), budget)
            if c:
                cur = c
        T = len(cur) - 1
        while T >= k and len(cur) <= MAXBMC:
            res, prog = solve_T(r, pattern, k, T, budget)
            if not res:
                break
            assert simulate(r, pattern, prog) == k
            cur = prog
            T = len(prog) - 1
        if len(cur) < len(progs[i]):
            progs[i] = cur
            write_out(outp, progs)
        print(f'{i}: r={r} k={k} {len(cur)} ({"unsat" if res is False else "timeout"}) {time.time()-t0:.1f}s', flush=True)
    print('gesamt', sum(len(p) for p in progs))


if __name__ == '__main__':
    main()
