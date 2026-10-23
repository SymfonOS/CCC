//! Segment search with a free start: find a short building segment C_1..C_L that
//! creates every pattern cell without ever putting matter on a forbidden cell and
//! without hitting a pattern cell three times. Such a segment is also valid in
//! reverse order, so copies are built by walking it forwards and backwards in turn.

use super::*;
use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashSet};

/// Incremental enumeration of start configurations (one ray per active emitter).
struct Enumerator<'a> {
    g: &'a Geometry,
    opts: Vec<Vec<u8>>, // candidate ring positions per emitter (off is implicit)
    cnt: Vec<u8>,
    cov: i32,  // pattern cells hit exactly twice
    fbad: i32, // forbidden cells hit exactly twice
    ptri: i32, // pattern cells hit three or more times
    cur: [Option<u8>; 6],
    home: [usize; 6],
    heap: BinaryHeap<Reverse<(i32, i32, [Option<u8>; 6])>>,
    cap: usize,
}

impl Enumerator<'_> {
    fn ray(&mut self, e: usize, p: u8, add: bool) {
        let g = self.g;
        for &c in &g.ray[e][p as usize] {
            let c = c as usize;
            let (old, new) = if add {
                self.cnt[c] += 1;
                (self.cnt[c] - 1, self.cnt[c])
            } else {
                self.cnt[c] -= 1;
                (self.cnt[c] + 1, self.cnt[c])
            };
            let (lo, hi) = (old.min(new), old.max(new));
            let sign = if add { 1 } else { -1 };
            if g.pat_idx[c] >= 0 {
                if (lo, hi) == (1, 2) {
                    self.cov += sign;
                } else if (lo, hi) == (2, 3) {
                    self.cov -= sign;
                    self.ptri += sign;
                }
            } else if g.forbidden[c] {
                if (lo, hi) == (1, 2) {
                    self.fbad += sign;
                } else if (lo, hi) == (2, 3) {
                    self.fbad -= sign;
                }
            }
        }
    }

    fn rec(&mut self, e: usize) {
        if e == 6 {
            if self.fbad == 0 && self.ptri == 0 && self.cov > 0 {
                let n = self.g.n;
                let setup = (0..6)
                    .filter_map(|i| self.cur[i].map(|p| ring_dist(p as usize, self.home[i], n) as i32))
                    .max()
                    .unwrap_or(0);
                let key = (self.cov, -setup, self.cur);
                if self.heap.len() < self.cap {
                    self.heap.push(Reverse(key));
                } else if self.heap.peek().is_some_and(|Reverse(min)| (min.0, min.1) < (key.0, key.1)) {
                    self.heap.pop();
                    self.heap.push(Reverse(key));
                }
            }
            return;
        }
        self.cur[e] = None;
        self.rec(e + 1);
        for i in 0..self.opts[e].len() {
            let p = self.opts[e][i];
            self.cur[e] = Some(p);
            self.ray(e, p, true);
            self.rec(e + 1);
            self.ray(e, p, false);
        }
        self.cur[e] = None;
    }
}

fn ring_dist(a: usize, b: usize, n: usize) -> usize {
    let d = (a + n - b) % n;
    d.min(n - d)
}

fn inverse(line: &[u8; 6]) -> [u8; 6] {
    std::array::from_fn(|e| match line[e] {
        b'F' => b'B',
        b'B' => b'F',
        c => c,
    })
}

/// Best start configurations: pattern cells created, no forbidden matter, no triple hits.
fn start_configs(g: &Geometry, r: usize, cap: usize) -> Vec<[Option<u8>; 6]> {
    let n = g.n;
    // per emitter: ring positions whose beam crosses the pattern, best lines first
    let mut opts: Vec<Vec<u8>> = (0..6)
        .map(|e| {
            let mut v: Vec<(usize, u8)> = (0..n)
                .filter_map(|p| {
                    let k = g.ray[e][p].iter().filter(|&&c| g.pat_idx[c as usize] >= 0).count();
                    (k > 0).then_some((k, p as u8))
                })
                .collect();
            v.sort_by(|a, b| b.0.cmp(&a.0));
            v.into_iter().map(|x| x.1).collect()
        })
        .collect();
    // keep the enumeration affordable
    while opts.iter().map(|o| o.len() as u64 + 1).product::<u64>() > 3_000_000 {
        let e = (0..6).max_by_key(|&e| opts[e].len()).unwrap();
        opts[e].pop();
    }
    let mut en = Enumerator {
        g,
        opts,
        cnt: vec![0; g.ncell],
        cov: 0,
        fbad: 0,
        ptri: 0,
        cur: [None; 6],
        home: home(r),
        heap: BinaryHeap::new(),
        cap,
    };
    en.rec(0);
    let mut v: Vec<_> = en.heap.into_iter().map(|Reverse(k)| k).collect();
    v.sort_by(|a, b| (b.0, b.1).cmp(&(a.0, a.1)));
    v.into_iter().map(|k| k.2).collect()
}

/// Turn a start configuration into a beam state; inactive emitters wait on the
/// line closest to the cells that are still missing.
fn start_state(g: &Geometry, r: usize, cfg: &[Option<u8>; 6]) -> St {
    let np = g.pat.len();
    let n = g.n;
    let mut cnt = vec![0u8; g.ncell];
    for e in 0..6 {
        if let Some(p) = cfg[e] {
            for &c in &g.ray[e][p as usize] {
                cnt[c as usize] += 1;
            }
        }
    }
    let mut have = [0u64; NW];
    for (pi, &c) in g.pat.iter().enumerate() {
        if cnt[c as usize] == 2 {
            have[pi / 64] |= 1 << (pi % 64);
        }
    }
    let h = home(r);
    let pos: [u8; 6] = std::array::from_fn(|e| match cfg[e] {
        Some(p) => p,
        None => (0..n)
            .filter(|&p| !g.ray[e][p].is_empty())
            .min_by_key(|&p| {
                let d: usize = (0..np)
                    .filter(|&pi| (have[pi / 64] >> (pi % 64)) & 1 == 0)
                    .map(|pi| g.dd[(e * n + p) * np + pi] as usize)
                    .sum();
                (d, ring_dist(p, h[e], n))
            })
            .unwrap_or(h[e]) as u8,
    });
    let act = (0..6).filter(|&e| cfg[e].is_some()).fold(0u8, |a, e| a | 1 << e);
    St { pos, act, have, junk: [0; NJ] }
}

/// Short building segment from a free start. Returns (start state, segment lines).
fn find_segment(g: &Geometry, r: usize, width: usize) -> Option<(St, Vec<[u8; 6]>)> {
    let np = g.pat.len();
    if np > NW * 64 {
        return None;
    }
    let n = g.n;
    let h = home(r);
    let movers = env_num("MOVERS", 4) as usize;
    let moves: Vec<[u8; 6]> = (0..4usize.pow(6))
        .map(|code| std::array::from_fn(|e| ((code >> (2 * e)) & 3) as u8))
        .filter(|m: &[u8; 6]| m.iter().filter(|&&x| x != 3).count() <= movers)
        .collect();
    let hw = 100_000i64;
    // SEGTRI=1: pattern cells may be destroyed by triple hits (reverse walk is checked by simulation)
    let allow_triple = env_num("SEGTRI", 1) != 0;
    let width = width * env_num("SEGW", 4).max(1) as usize;
    let starts: Vec<St> = start_configs(g, r, 4 * width.max(16)).iter().map(|c| start_state(g, r, c)).collect();
    if starts.is_empty() {
        return None;
    }
    let setup = |st: &St| (0..6).map(|e| ring_dist(st.pos[e] as usize, h[e], n)).max().unwrap_or(0);
    // nodes: (parent, line, root start index)
    let mut nodes: Vec<(u32, [u8; 6], u32)> = Vec::new();
    let mut visited = HashSet::new();
    let mut ranked: Vec<(i64, usize)> = starts
        .iter()
        .enumerate()
        .map(|(i, st)| (popcount(&st.have) as i64 * hw + potential(g, st) - setup(st) as i64, i))
        .collect();
    ranked.sort_by(|a, b| b.0.cmp(&a.0));
    let mut beam: Vec<(u32, St)> = Vec::new();
    for &(_, i) in ranked.iter().take(width) {
        if popcount(&starts[i].have) == np {
            return Some((starts[i], Vec::new()));
        }
        if visited.insert(hash_state(&starts[i])) {
            nodes.push((u32::MAX, [b'-'; 6], i as u32));
            beam.push(((nodes.len() - 1) as u32, starts[i]));
        }
    }
    let mut cnt = vec![0u8; g.ncell];
    let mut touched: Vec<u16> = Vec::new();
    for _tick in 0..40 * np + 400 {
        let mut cands: Vec<(i64, u32, [u8; 6], St)> = Vec::new();
        let mut finished: Vec<(usize, u32, [u8; 6], St)> = Vec::new();
        for (bi, (node, st)) in beam.iter().enumerate() {
            for mv in &moves {
                let mut ns = *st;
                for e in 0..6 {
                    match mv[e] {
                        0 => ns.pos[e] = ((ns.pos[e] as usize + 1) % n) as u8,
                        1 => ns.pos[e] = ((ns.pos[e] as usize + n - 1) % n) as u8,
                        2 => ns.act ^= 1 << e,
                        _ => {}
                    }
                }
                touched.clear();
                for e in 0..6 {
                    if (ns.act >> e) & 1 != 0 {
                        for &c in &g.ray[e][ns.pos[e] as usize] {
                            if cnt[c as usize] == 0 {
                                touched.push(c);
                            }
                            cnt[c as usize] += 1;
                        }
                    }
                }
                let mut ok = true;
                for &c in &touched {
                    let v = cnt[c as usize];
                    let i = g.pat_idx[c as usize];
                    if v == 2 {
                        if g.forbidden[c as usize] {
                            ok = false;
                        }
                        if i >= 0 {
                            ns.have[i as usize / 64] |= 1 << (i as usize % 64);
                        }
                    } else if v >= 3 && i >= 0 {
                        if allow_triple {
                            ns.have[i as usize / 64] &= !(1 << (i as usize % 64));
                        } else {
                            ok = false;
                        }
                    }
                    cnt[c as usize] = 0;
                }
                if !ok {
                    continue;
                }
                let line: [u8; 6] = std::array::from_fn(|e| INSTR[mv[e] as usize]);
                let have = popcount(&ns.have);
                if have == np {
                    let root = nodes[*node as usize].2;
                    finished.push((setup(&starts[root as usize]), bi as u32, line, ns));
                    continue;
                }
                cands.push((have as i64 * hw + potential(g, &ns), bi as u32, line, ns));
                if cands.len() >= 8 * width + 4096 {
                    let keep = 3 * width;
                    cands.select_nth_unstable_by(keep, |a, b| b.0.cmp(&a.0));
                    cands.truncate(keep);
                }
            }
        }
        if !finished.is_empty() {
            finished.sort_by_key(|f| f.0);
            let (_, bi, line, _) = finished[0];
            let parent = beam[bi as usize].0;
            let root = nodes[parent as usize].2;
            let mut seq = vec![line];
            let mut node = parent;
            while nodes[node as usize].0 != u32::MAX {
                seq.push(nodes[node as usize].1);
                node = nodes[node as usize].0;
            }
            seq.reverse();
            return Some((starts[root as usize], seq));
        }
        cands.sort_unstable_by(|a, b| b.0.cmp(&a.0));
        let mut next: Vec<(u32, St)> = Vec::with_capacity(width);
        for (_, bi, line, ns) in cands {
            if next.len() >= width {
                break;
            }
            if !visited.insert(hash_state(&ns)) {
                continue;
            }
            let parent = beam[bi as usize].0;
            nodes.push((parent, line, nodes[parent as usize].2));
            next.push(((nodes.len() - 1) as u32, ns));
        }
        if next.is_empty() {
            return None;
        }
        beam = next;
    }
    None
}

/// Program: move into place (inactive), activate = C_1, walk the segment, then
/// alternate backward / forward walks until `k` copies are submitted.
pub fn segment_program(g: &Geometry, r: usize, pattern: &[Vec<u8>], k: u32, width: usize) -> Option<Vec<[u8; 6]>> {
    let Some((start, seg)) = find_segment(g, r, width) else {
        if std::env::var("SEGDBG").is_ok() {
            eprintln!("seg: np={} k={} keine Lösung", g.pat.len(), k);
        }
        return None;
    };
    let n = g.n;
    let h = home(r);
    let moves: Vec<Vec<u8>> = (0..6)
        .map(|e| {
            let t = start.pos[e] as usize;
            let fwd = (t + n - h[e]) % n;
            if fwd <= n - fwd {
                vec![b'F'; fwd]
            } else {
                vec![b'B'; n - fwd]
            }
        })
        .collect();
    let d = moves.iter().map(Vec::len).max().unwrap_or(0);
    let mut prog: Vec<[u8; 6]> = (0..d).map(|i| std::array::from_fn(|e| *moves[e].get(i).unwrap_or(&b'-'))).collect();
    prog.push(std::array::from_fn(|e| if (start.act >> e) & 1 != 0 { b'T' } else { b'-' }));
    prog.extend(&seg);
    let back: Vec<[u8; 6]> = seg.iter().rev().map(inverse).collect();
    for c in 1..k {
        prog.push([b'-'; 6]);
        prog.extend(if c % 2 == 1 { &back } else { &seg });
    }
    let subs = submissions(r, pattern, &prog, k as usize);
    if subs.len() < k as usize {
        return None;
    }
    prog.truncate(subs[k as usize - 1] + 1);
    let prog = polish(r, pattern, k, prog);
    if std::env::var("SEGDBG").is_ok() {
        eprintln!("seg: np={} k={} L={} setup={} len={}", g.pat.len(), k, seg.len() + 1, d, prog.len());
    }
    (simulate(r, pattern, &prog) == k).then_some(prog)
}

/// Per tick: (some pattern cell was newly created, pattern was submitted).
fn trace(r: usize, pattern: &[Vec<u8>], prog: &[[u8; 6]]) -> Vec<(bool, bool)> {
    let mut grid = empty_board(r);
    let w = grid[0].len();
    let cells: Vec<(usize, usize)> = pattern
        .iter()
        .enumerate()
        .flat_map(|(y, row)| row.iter().enumerate().filter(|(_, &c)| c == b'M').map(move |(x, _)| (y, x)))
        .collect();
    let ring = ring(r);
    let n = ring.len();
    let mut idx = home(r);
    let mut active = [false; 6];
    let mut hits = vec![0u8; grid.len() * w];
    let mut touched: Vec<(usize, usize)> = Vec::new();
    let mut out = Vec::with_capacity(prog.len());
    for line in prog {
        for e in 0..6 {
            match line[e] {
                b'F' => idx[e] = (idx[e] + 1) % n,
                b'B' => idx[e] = (idx[e] + n - 1) % n,
                b'T' => active[e] = !active[e],
                _ => {}
            }
        }
        touched.clear();
        for e in 0..6 {
            if !active[e] {
                continue;
            }
            let (mut y, mut x) = ring[idx[e]];
            loop {
                y += DIRS[e].0;
                x += DIRS[e].1;
                let c = get(&grid, y, x);
                if c != b'O' && c != b'M' {
                    break;
                }
                let k = y as usize * w + x as usize;
                if hits[k] == 0 {
                    touched.push((y as usize, x as usize));
                }
                hits[k] += 1;
            }
        }
        let mut created = false;
        for &(y, x) in &touched {
            let k = y * w + x;
            match hits[k] {
                2 => {
                    if pattern[y][x] == b'M' && grid[y][x] != b'M' {
                        created = true;
                    }
                    grid[y][x] = b'M';
                }
                3.. => grid[y][x] = b'O',
                _ => {}
            }
            hits[k] = 0;
        }
        let complete = !cells.is_empty()
            && cells.iter().all(|&(y, x)| grid[y][x] == b'M')
            && cells.iter().all(|&(y, x)| {
                NEIGHBORS.iter().all(|&(dy, dx)| {
                    let (ny, nx) = (y as i64 + dy, x as i64 + dx);
                    get(&grid, ny, nx) != b'M' || get(pattern, ny, nx) == b'M'
                })
            });
        if complete {
            for &(y, x) in &cells {
                grid[y][x] = b'O';
            }
        }
        out.push((created, complete));
    }
    out
}

/// Reuse the shortest copy of an existing program: keep everything up to that copy,
/// then walk its building segment backwards and forwards for the remaining copies.
pub fn reuse_best_copy(r: usize, pattern: &[Vec<u8>], k: u32, prog: &[[u8; 6]]) -> Option<Vec<[u8; 6]>> {
    if k < 2 {
        return None;
    }
    let tr = trace(r, pattern, prog);
    // (first creation tick, submission tick) per copy
    let mut segs = Vec::new();
    let mut start: Option<usize> = None;
    for (t, &(created, done)) in tr.iter().enumerate() {
        if created && start.is_none() {
            start = Some(t);
        }
        if done {
            segs.push((start.unwrap_or(t), t));
            start = None;
        }
    }
    if segs.len() != k as usize {
        return None;
    }
    let mut best: Option<Vec<[u8; 6]>> = None;
    for (j, &(a, b)) in segs.iter().enumerate().take(k as usize - 1) {
        let fwd: Vec<[u8; 6]> = prog[a + 1..=b].to_vec();
        let back: Vec<[u8; 6]> = fwd.iter().rev().map(inverse).collect();
        let mut cand: Vec<[u8; 6]> = prog[..=b].to_vec();
        for c in (j + 1)..k as usize {
            cand.push([b'-'; 6]);
            cand.extend(if (c - j) % 2 == 1 { &back } else { &fwd });
        }
        let subs = submissions(r, pattern, &cand, k as usize);
        if subs.len() < k as usize {
            continue;
        }
        cand.truncate(subs[k as usize - 1] + 1);
        if cand.len() < best.as_ref().map_or(prog.len(), |x| x.len()) {
            best = Some(cand);
        }
    }
    let best = polish(r, pattern, k, best?);
    (simulate(r, pattern, &best) == k && best.len() < prog.len()).then_some(best)
}
