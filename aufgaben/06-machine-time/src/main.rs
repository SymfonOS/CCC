mod endgame;

use std::{fs, path::Path};

mod segment;
mod extbeam;

const DIRS: [(i64, i64); 6] = [(0, 2), (1, 1), (1, -1), (0, -2), (-1, -1), (-1, 1)];
const NEIGHBORS: [(i64, i64); 6] = [(0, 2), (0, -2), (1, 1), (1, -1), (-1, 1), (-1, -1)];
const INSTR: [u8; 4] = [b'F', b'B', b'T', b'-'];

fn empty_board(r: usize) -> Vec<Vec<u8>> {
    (0..=2 * r)
        .map(|k| {
            let m = k.min(2 * r - k);
            let mut row = vec![b'-'; 4 * r + 1];
            for i in 0..r + 1 + m {
                let edge = k == 0 || k == 2 * r || i == 0 || i == r + m;
                row[r - m + 2 * i] = if edge { b'*' } else { b'O' };
            }
            row
        })
        .collect()
}

/// Border positions in clockwise order, starting at the top-left corner.
fn ring(r: usize) -> Vec<(i64, i64)> {
    let r = r as i64;
    let mut v = Vec::new();
    v.extend((0..r).map(|i| (0, r + 2 * i)));
    v.extend((0..r).map(|i| (i, 3 * r + i)));
    v.extend((0..r).map(|i| (r + i, 4 * r - i)));
    v.extend((0..r).map(|i| (2 * r, 3 * r - 2 * i)));
    v.extend((0..r).map(|i| (2 * r - i, r - i)));
    v.extend((0..r).map(|i| (r - i, i)));
    v
}

fn get(grid: &[Vec<u8>], y: i64, x: i64) -> u8 {
    if y < 0 || x < 0 || y as usize >= grid.len() || x as usize >= grid[0].len() {
        0
    } else {
        grid[y as usize][x as usize]
    }
}

fn home(r: usize) -> [usize; 6] {
    [5 * r, 0, r, 2 * r, 3 * r, 4 * r]
}

/// Reference simulator (level 5): number of submitted patterns.
fn simulate(r: usize, pattern: &[Vec<u8>], prog: &[[u8; 6]]) -> u32 {
    let mut grid = empty_board(r);
    let cells: Vec<(i64, i64)> = pattern
        .iter()
        .enumerate()
        .flat_map(|(y, row)| row.iter().enumerate().filter(|(_, &c)| c == b'M').map(move |(x, _)| (y as i64, x as i64)))
        .collect();
    let in_pattern = |y: i64, x: i64| get(pattern, y, x) == b'M';
    let ring = ring(r);
    let n = ring.len();
    let mut idx = home(r);
    let mut active = [false; 6];
    let mut count = 0;
    for line in prog {
        for e in 0..6 {
            match line[e] {
                b'F' => idx[e] = (idx[e] + 1) % n,
                b'B' => idx[e] = (idx[e] + n - 1) % n,
                b'T' => active[e] = !active[e],
                _ => {}
            }
        }
        let mut hits = vec![vec![0u32; grid[0].len()]; grid.len()];
        for e in 0..6 {
            if !active[e] {
                continue;
            }
            let (mut y, mut x) = ring[idx[e]];
            let (dy, dx) = DIRS[e];
            loop {
                y += dy;
                x += dx;
                let c = get(&grid, y, x);
                if c != b'O' && c != b'M' {
                    break;
                }
                hits[y as usize][x as usize] += 1;
            }
        }
        for (y, row) in grid.iter_mut().enumerate() {
            for (x, cell) in row.iter_mut().enumerate() {
                match hits[y][x] {
                    2 => *cell = b'M',
                    3.. => *cell = b'O',
                    _ => {}
                }
            }
        }
        if cells.is_empty() || !cells.iter().all(|&(y, x)| get(&grid, y, x) == b'M') {
            continue;
        }
        let clean = cells.iter().all(|&(y, x)| {
            NEIGHBORS
                .iter()
                .all(|&(dy, dx)| get(&grid, y + dy, x + dx) != b'M' || in_pattern(y + dy, x + dx))
        });
        if clean {
            for &(y, x) in &cells {
                grid[y as usize][x as usize] = b'O';
            }
            count += 1;
        }
    }
    count
}

struct Geometry {
    n: usize,
    ray: Vec<Vec<Vec<u16>>>, // ray[emitter][ring position] -> hit cells
    pat: Vec<u16>,           // pattern cell ids
    pat_idx: Vec<i32>,       // cell id -> index in pat, or -1
    f_idx: Vec<i32>,         // cell id -> index among forbidden cells, or -1
    nf: usize,
    forbidden: Vec<bool>,    // non-pattern cells adjacent to the pattern
    ncell: usize,
    dd: Vec<u8>,             // dd[(e * n + pos) * np + pi]: ticks until emitter e is aligned with pattern cell pi
}

fn geometry(r: usize, pattern: &[Vec<u8>]) -> Geometry {
    let board = empty_board(r);
    let w = board[0].len();
    let ring = ring(r);
    let n = ring.len();
    let mut ray = vec![vec![Vec::new(); n]; 6];
    for e in 0..6 {
        for p in 0..n {
            let (mut y, mut x) = ring[p];
            loop {
                y += DIRS[e].0;
                x += DIRS[e].1;
                if get(&board, y, x) != b'O' {
                    break;
                }
                ray[e][p].push((y as usize * w + x as usize) as u16);
            }
        }
    }
    let ncell = board.len() * w;
    let mut pat = Vec::new();
    let mut pat_idx = vec![-1i32; ncell];
    for (y, row) in pattern.iter().enumerate() {
        for (x, &c) in row.iter().enumerate() {
            if c == b'M' {
                pat_idx[y * w + x] = pat.len() as i32;
                pat.push((y * w + x) as u16);
            }
        }
    }
    let mut forbidden = vec![false; ncell];
    for &c in &pat {
        let (y, x) = ((c as usize / w) as i64, (c as usize % w) as i64);
        for (dy, dx) in NEIGHBORS {
            let (ny, nx) = (y + dy, x + dx);
            if get(&board, ny, nx) == b'O' && get(pattern, ny, nx) != b'M' {
                forbidden[ny as usize * w + nx as usize] = true;
            }
        }
    }
    // ring position from which emitter e's beam passes through each cell
    let mut origin = vec![vec![255u8; ncell]; 6];
    for e in 0..6 {
        for p in 0..n {
            for &c in &ray[e][p] {
                origin[e][c as usize] = p as u8;
            }
        }
    }
    let np = pat.len();
    let mut dd = vec![99u8; 6 * n * np];
    for e in 0..6 {
        for pos in 0..n {
            for (pi, &c) in pat.iter().enumerate() {
                let o = origin[e][c as usize];
                if o != 255 {
                    let d = (pos as i64 - o as i64).unsigned_abs() as usize;
                    dd[(e * n + pos) * np + pi] = d.min(n - d) as u8;
                }
            }
        }
    }
    let mut f_idx = vec![-1i32; ncell];
    let mut nf = 0;
    for c in 0..ncell {
        if forbidden[c] {
            f_idx[c] = nf as i32;
            nf += 1;
        }
    }
    Geometry { n, ray, pat, pat_idx, f_idx, nf, forbidden, ncell, dd }
}

/// Build the pattern `k` times using only the emitter pair (a, b).
fn plan(g: &Geometry, r: usize, a: usize, b: usize, k: u32) -> Option<Vec<[u8; 6]>> {
    let n = g.n;
    // cells created by the pair when both beams are active
    let mut mark = vec![u32::MAX; g.ncell];
    let mut cross: Vec<(Vec<u16>, bool)> = Vec::with_capacity(n * n);
    for pa in 0..n {
        for pb in 0..n {
            for &c in &g.ray[a][pa] {
                mark[c as usize] = (pa * n + pb) as u32;
            }
            let cells: Vec<u16> = g.ray[b][pb].iter().copied().filter(|&c| mark[c as usize] == (pa * n + pb) as u32).collect();
            let bad = cells.iter().any(|&c| g.forbidden[c as usize]);
            cross.push((cells, bad));
            for &c in &g.ray[a][pa] {
                mark[c as usize] = u32::MAX;
            }
        }
    }
    let h = home(r);
    let (mut pa, mut pb, mut aa, mut ab) = (h[a], h[b], false, false);
    let mut prog: Vec<[u8; 6]> = Vec::new();
    let nstates = n * n * 4;
    let mut seen = vec![0u32; nstates];
    let mut par = vec![0u32; nstates];
    let mut how = vec![0u8; nstates];
    let mut stamp = 0u32;
    let mut queue: Vec<u32> = Vec::new();
    let step = |pa: usize, pb: usize, aa: bool, ab: bool, ia: usize, ib: usize| {
        let mut npa = pa;
        let mut npb = pb;
        let (mut naa, mut nab) = (aa, ab);
        match ia {
            0 => npa = (pa + 1) % n,
            1 => npa = (pa + n - 1) % n,
            2 => naa = !aa,
            _ => {}
        }
        match ib {
            0 => npb = (pb + 1) % n,
            1 => npb = (pb + n - 1) % n,
            2 => nab = !ab,
            _ => {}
        }
        (npa, npb, naa, nab)
    };
    let enc = |pa: usize, pb: usize, aa: bool, ab: bool| ((pa * n + pb) << 2) | aa as usize | ((ab as usize) << 1);
    for _ in 0..k {
        let mut have = vec![false; g.pat.len()];
        let mut remaining = g.pat.len();
        while remaining > 0 {
            stamp += 1;
            queue.clear();
            let s0 = enc(pa, pb, aa, ab);
            seen[s0] = stamp;
            queue.push(s0 as u32);
            let mut qi = 0;
            let mut found: Option<(usize, usize)> = None; // (state, instruction code)
            'bfs: while qi < queue.len() {
                let s = queue[qi] as usize;
                qi += 1;
                let (spb, spa) = ((s >> 2) % n, (s >> 2) / n);
                let (saa, sab) = (s & 1 != 0, s & 2 != 0);
                for ia in 0..4 {
                    for ib in 0..4 {
                        let (npa, npb, naa, nab) = step(spa, spb, saa, sab, ia, ib);
                        let mut progress = false;
                        if naa && nab {
                            let (cells, bad) = &cross[npa * n + npb];
                            if *bad {
                                continue;
                            }
                            progress = cells.iter().any(|&c| {
                                let i = g.pat_idx[c as usize];
                                i >= 0 && !have[i as usize]
                            });
                        }
                        if progress {
                            found = Some((s, ia * 4 + ib));
                            break 'bfs;
                        }
                        let ns = enc(npa, npb, naa, nab);
                        if seen[ns] != stamp {
                            seen[ns] = stamp;
                            par[ns] = s as u32;
                            how[ns] = (ia * 4 + ib) as u8;
                            queue.push(ns as u32);
                        }
                    }
                }
            }
            let (last, code) = found?;
            // reconstruct the instruction sequence
            let mut seq = vec![code];
            let mut s = last;
            while s != s0 {
                seq.push(how[s] as usize);
                s = par[s] as usize;
            }
            seq.reverse();
            for code in seq {
                let (ia, ib) = (code / 4, code % 4);
                let (npa, npb, naa, nab) = step(pa, pb, aa, ab, ia, ib);
                pa = npa;
                pb = npb;
                aa = naa;
                ab = nab;
                let mut line = [b'-'; 6];
                line[a] = INSTR[ia];
                line[b] = INSTR[ib];
                prog.push(line);
                if aa && ab {
                    for &c in &cross[pa * n + pb].0 {
                        let i = g.pat_idx[c as usize];
                        if i >= 0 && !have[i as usize] {
                            have[i as usize] = true;
                            remaining -= 1;
                        }
                    }
                }
            }
        }
    }
    Some(prog)
}


const NW: usize = 5;

const NJ: usize = 4;

#[derive(Clone, Copy)]
struct St {
    pos: [u8; 6],
    act: u8,
    have: [u64; NW],
    junk: [u64; NJ], // forbidden cells currently holding matter
}

fn popcount(h: &[u64; NW]) -> usize {
    h.iter().map(|w| w.count_ones() as usize).sum()
}

fn env_num(name: &str, default: i64) -> i64 {
    std::env::var(name).ok().and_then(|v| v.parse().ok()).unwrap_or(default)
}

/// Weight of a missing pattern cell that is `d` ticks away from being creatable.
fn weight(d: i64, mode: i64) -> i64 {
    match mode {
        1 => 100 / (1 + d),
        2 => 100 >> d.min(20),
        _ => 100 / ((1 + d) * (1 + d)),
    }
}

fn potential(g: &Geometry, st: &St) -> i64 {
    potential_mode(g, st, POT_MODE.with(|m| *m), endgame::family_potential())
}

fn potential_with(g: &Geometry, st: &St, pfam: bool) -> i64 {
    potential_mode(g, st, POT_MODE.with(|m| *m), pfam)
}

thread_local! {
    static NOISE: i64 = env_num("NOISE", 0);
    static SEED: u64 = env_num("SEED", 0) as u64;
    static POT_MODE: i64 = env_num("POT", 0);
    static HAVE_W: i64 = env_num("HAVE_W", 100_000);
}

fn potential_mode(g: &Geometry, st: &St, mode: i64, pfam: bool) -> i64 {
    let np = g.pat.len();
    let n = g.n;
    let rows: [&[u8]; 6] = std::array::from_fn(|e| &g.dd[(e * n + st.pos[e] as usize) * np..][..np]);
    let off: [u8; 6] = std::array::from_fn(|e| ((st.act >> e) & 1 == 0) as u8);
    let mut hist = [0u32; 64];
    for pi in 0..np {
        if (st.have[pi / 64] >> (pi % 64)) & 1 != 0 {
            continue;
        }
        // three smallest of the six alignment distances (branchless insertion)
        let (mut m1, mut m2, mut m3) = (u8::MAX, u8::MAX, u8::MAX);
        for e in 0..6 {
            let v = rows[e][pi].saturating_add(off[e]);
            let lo1 = m1.min(v);
            let hi1 = m1.max(v);
            let lo2 = m2.min(hi1);
            let hi2 = m2.max(hi1);
            m3 = m3.min(hi2);
            m1 = lo1;
            m2 = lo2;
        }
        let d = if pfam {
            // creation needs two different families: second smallest of the per-family minima
            let f: [u8; 3] = std::array::from_fn(|k| {
                rows[k][pi].saturating_add(off[k]).min(rows[k + 3][pi].saturating_add(off[k + 3]))
            });
            let lo = f[0].min(f[1]).min(f[2]);
            let hi = f[0].max(f[1]).max(f[2]);
            let mid = f[0] as u16 + f[1] as u16 + f[2] as u16 - lo as u16 - hi as u16;
            if hi == 0 { 3 } else { (mid as u8).min(63) }
        } else if m3 == 0 {
            3
        } else {
            m2.min(63)
        };
        hist[d as usize] += 1;
    }
    hist.iter().enumerate().map(|(d, &c)| c as i64 * weight(d as i64, mode)).sum()
}

fn hash_state(st: &St) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    let mut mix = |v: u64| {
        h ^= v;
        h = h.wrapping_mul(0x100000001b3);
        h ^= h >> 29;
    };
    for e in 0..6 {
        mix(st.pos[e] as u64);
    }
    mix(st.act as u64);
    for w in st.have {
        mix(w);
    }
    for w in st.junk {
        mix(w);
    }
    h
}

/// Parameters of one beam-search run for a single copy.
#[derive(Clone)]
struct BeamParams {
    width: usize,
    jw: i64,
    hw: i64,
    la: usize,
    law: i64,
    la_full: bool,
    noise: i64,
    seed: u64,
    pfam: bool,
    keep_have: bool, // continue with the pattern cells already present (re-planning from a state)
}

/// Beam search for one copy starting at `cur` (pattern absent). Returns the lines,
/// the final state and the number of pattern cells present after every line.
#[allow(clippy::too_many_arguments)]
fn search_copy(
    g: &Geometry,
    moves: &[[u8; 6]],
    la_moves: &[[u8; 6]],
    la_full_moves: &[[u8; 6]],
    mut cur: St,
    p: &BeamParams,
    cnt: &mut [u8],
    touched: &mut Vec<u16>,
    configs: Option<&endgame::ConfigList>,
) -> Option<(Vec<[u8; 6]>, St, Vec<usize>)> {
    let np = g.pat.len();
    let n = g.n;
    let width = p.width;
    let (jw, hw, la, law, noise) = (p.jw, p.hw, p.la, p.law, p.noise);
    let la_set = if p.la_full { la_full_moves } else { la_moves };
    let mut rng: u64 = p.seed.wrapping_mul(0x9E3779B97F4A7C15) ^ (np as u64).wrapping_mul(0x632BE59BD9B4E019) ^ g.pat[0] as u64 ^ 0xA5A5_5A5A;
    let mut rand = move || {
        rng ^= rng << 13;
        rng ^= rng >> 7;
        rng ^= rng << 17;
        rng
    };
    if !p.keep_have {
        cur.have = [0; NW];
    }
    let mut nodes: Vec<(u32, [u8; 6], u16)> = vec![(u32::MAX, [b'-'; 6], 0)];
    let mut beam: Vec<(u32, St)> = vec![(0, cur)];
    let mut visited = std::collections::HashSet::new();
    visited.insert(hash_state(&cur));
    let limit = 40 * np + 400;
    let mut done: Option<(u32, St)> = None;
    for _tick in 0..limit {
        let mut cands: Vec<(i64, u32, [u8; 6], St)> = Vec::new();
        let mut finished: Vec<(i64, u32, [u8; 6], St)> = Vec::new();
        for (bi, (_, st)) in beam.iter().enumerate() {
            for mv in moves {
                let mut ns = *st;
                let mut ok = true;
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
                for &c in touched.iter() {
                    let v = cnt[c as usize];
                    if v == 2 {
                        if g.forbidden[c as usize] {
                            if jw == 0 {
                                ok = false;
                            } else {
                                let f = g.f_idx[c as usize] as usize;
                                ns.junk[f / 64] |= 1 << (f % 64);
                            }
                        }
                        let i = g.pat_idx[c as usize];
                        if i >= 0 {
                            ns.have[i as usize / 64] |= 1 << (i as usize % 64);
                        }
                    } else if v >= 3 {
                        if jw != 0 && g.forbidden[c as usize] {
                            let f = g.f_idx[c as usize] as usize;
                            ns.junk[f / 64] &= !(1 << (f % 64));
                        }
                        let i = g.pat_idx[c as usize];
                        if i >= 0 {
                            ns.have[i as usize / 64] &= !(1 << (i as usize % 64));
                        }
                    }
                    cnt[c as usize] = 0;
                }
                if !ok {
                    continue;
                }
                let line: [u8; 6] = std::array::from_fn(|e| INSTR[mv[e] as usize]);
                let have = popcount(&ns.have);
                let junk: usize = ns.junk.iter().map(|w| w.count_ones() as usize).sum();
                if have == np && junk == 0 {
                    let mut fresh = ns;
                    fresh.have = [0; NW];
                    finished.push((-potential_with(g, &fresh, p.pfam), bi as u32, line, ns));
                    continue;
                }
                let mut score = have as i64 * hw - junk as i64 * jw + potential_with(g, &ns, p.pfam);
                if noise > 0 {
                    score += (rand() % noise as u64) as i64;
                }
                cands.push((score, bi as u32, line, ns));
                if cands.len() >= 8 * width * 16 + 4096 {
                    let keep = 3 * width * 16;
                    cands.select_nth_unstable_by(keep, |a, b| b.0.cmp(&a.0));
                    cands.truncate(keep);
                }
            }
        }
        if !finished.is_empty() {
            finished.sort_by_key(|f| f.0);
            let (_, bi, line, ns) = finished[0];
            nodes.push((beam[bi as usize].0, line, np as u16));
            done = Some(((nodes.len() - 1) as u32, ns));
            break;
        }
        cands.sort_unstable_by(|a, b| b.0.cmp(&a.0));
        if let Some(list) = configs {
            // steer towards reachable configurations that create many missing cells at once
            let tw = endgame::target_weight();
            let m = cands.len().min(2 * width);
            for c in cands[..m].iter_mut() {
                c.0 += endgame::rate_term(g, list, &c.3) * tw / 100;
            }
            cands[..m].sort_unstable_by(|a, b| b.0.cmp(&a.0));
        }
        if la > 0 {
            // re-rank the best candidates by the gain the following line could achieve
            let m = cands.len().min(la * width);
            for c in cands[..m].iter_mut() {
                let gn = endgame::best_next_gain(g, &c.3, la_set, cnt, touched);
                c.0 += gn as i64 * hw * law / 100;
            }
            cands[..m].sort_unstable_by(|a, b| b.0.cmp(&a.0));
        }
        let best_have = cands.first().map_or(0, |c| popcount(&c.3.have));
        let cur_width = endgame::width(width, best_have, np);
        let mut next: Vec<(u32, St)> = Vec::with_capacity(cur_width);
        let div = endgame::diversity();
        let mut per_have: std::collections::HashMap<[u64; NW], usize> = std::collections::HashMap::new();
        for (_, bi, line, ns) in cands {
            if next.len() >= cur_width {
                break;
            }
            if div > 0 {
                let c = per_have.entry(ns.have).or_insert(0);
                if *c >= div {
                    continue;
                }
                *c += 1;
            }
            if !visited.insert(hash_state(&ns)) {
                continue;
            }
            nodes.push((beam[bi as usize].0, line, popcount(&ns.have) as u16));
            next.push(((nodes.len() - 1) as u32, ns));
        }
        if next.is_empty() {
            return None;
        }
        beam = next;
    }
    let (mut node, ns) = done?;
    let mut seq = Vec::new();
    let mut haves = Vec::new();
    while node != 0 {
        seq.push(nodes[node as usize].1);
        haves.push(nodes[node as usize].2 as usize);
        node = nodes[node as usize].0;
    }
    seq.reverse();
    haves.reverse();
    Some((seq, ns, haves))
}

/// Beam search over all six emitters; builds the pattern `k` times.
/// With VARIANTS > 1 every copy is searched with several parameter variants and the
/// shortest copy is kept.
fn plan_beam(g: &Geometry, r: usize, k: u32, width: usize) -> Option<Vec<[u8; 6]>> {
    plan_beam_from(g, r, k, width, None)
}

/// Like `plan_beam`, but optionally continues from a given state (positions, active flags,
/// pattern cells already built for the first copy).
fn plan_beam_from(g: &Geometry, r: usize, k: u32, width: usize, start: Option<St>) -> Option<Vec<[u8; 6]>> {
    let np = g.pat.len();
    if np > NW * 64 {
        return None;
    }
    let h = home(r);
    let mut cur = start.unwrap_or(St { pos: std::array::from_fn(|e| h[e] as u8), act: 0, have: [0; NW], junk: [0; NJ] });
    let keep_first = start.is_some();
    // JW > 0: matter on forbidden cells is allowed temporarily (penalty JW each) and must be destroyed again
    let jw = if g.nf <= NJ * 64 { env_num("JW", 0) } else { 0 };
    // moves: up to `movers` emitters act; op 0=F 1=B 2=T, 3 = idle
    let movers: usize = std::env::var("MOVERS").ok().and_then(|v| v.parse().ok()).unwrap_or(4);
    let mut moves: Vec<[u8; 6]> = Vec::new();
    for code in 0..4usize.pow(6) {
        let mut m = [3u8; 6];
        let mut c = code;
        let mut acting = 0;
        for e in 0..6 {
            m[e] = (c % 4) as u8;
            c /= 4;
            if m[e] != 3 {
                acting += 1;
            }
        }
        if acting <= movers {
            moves.push(m);
        }
    }
    let base = BeamParams {
        width,
        jw,
        hw: HAVE_W.with(|w| *w),
        la: endgame::lookahead_factor(),
        law: endgame::lookahead_weight(),
        la_full: false,
        noise: NOISE.with(|v| *v),
        seed: SEED.with(|v| *v),
        pfam: endgame::family_potential(),
        keep_have: false,
    };
    let la_moves = endgame::lookahead_moves();
    let la_full_moves: Vec<[u8; 6]> = moves.clone();
    let variants = endgame::variants(&base);
    let configs = (endgame::target_weight() > 0).then(|| endgame::build_configs(g));
    let mut cnt = vec![0u8; g.ncell];
    let mut touched: Vec<u16> = Vec::new();
    if endgame::whole_program_variants() {
        // every variant builds all k copies on its own; keep the shortest program
        let mut best: Option<Vec<[u8; 6]>> = None;
        for v in &variants {
            let mut st = cur;
            let mut prog: Vec<[u8; 6]> = Vec::new();
            let mut ok = true;
            for copy in 0..k {
                let mut pv = v.clone();
                pv.seed = pv.seed.wrapping_add(copy as u64 * 7919);
                pv.keep_have = copy == 0 && keep_first;
                match search_copy(g, &moves, &la_moves, &la_full_moves, st, &pv, &mut cnt, &mut touched, configs.as_ref()) {
                    Some((seq, ns, _)) => {
                        prog.extend(seq);
                        st = ns;
                    }
                    None => {
                        ok = false;
                        break;
                    }
                }
                if best.as_ref().is_some_and(|b| prog.len() >= b.len()) {
                    ok = false;
                    break;
                }
            }
            if ok {
                best = Some(prog);
            }
        }
        return best;
    }
    let mut prog: Vec<[u8; 6]> = Vec::new();
    for copy in 0..k {
        let mut best: Option<(Vec<[u8; 6]>, St, Vec<usize>, i64)> = None;
        for v in &variants {
            let mut pv = v.clone();
            pv.seed = pv.seed.wrapping_add(copy as u64 * 7919);
            pv.keep_have = copy == 0 && keep_first;
            if let Some((seq, ns, haves)) = search_copy(g, &moves, &la_moves, &la_full_moves, cur, &pv, &mut cnt, &mut touched, configs.as_ref()) {
                let mut fresh = ns;
                fresh.have = [0; NW];
                let pot = potential(g, &fresh);
                let better = match &best {
                    None => true,
                    Some((b, _, _, bp)) => seq.len() < b.len() || (seq.len() == b.len() && pot > *bp),
                };
                if better {
                    best = Some((seq, ns, haves, pot));
                }
            }
        }
        let (seq, ns, haves, _) = best?;
        endgame::record_copy(&haves, np);
        prog.extend(seq);
        cur = ns;
    }
    Some(prog)
}

/// Ticks at which the pattern is submitted (stops after `limit` submissions).
fn submissions(r: usize, pattern: &[Vec<u8>], prog: &[[u8; 6]], limit: usize) -> Vec<usize> {
    let mut grid = empty_board(r);
    let cells: Vec<(i64, i64)> = pattern
        .iter()
        .enumerate()
        .flat_map(|(y, row)| row.iter().enumerate().filter(|(_, &c)| c == b'M').map(move |(x, _)| (y as i64, x as i64)))
        .collect();
    let in_pattern = |y: i64, x: i64| get(pattern, y, x) == b'M';
    let ring = ring(r);
    let n = ring.len();
    let mut idx = home(r);
    let mut active = [false; 6];
    let mut subs = Vec::new();
    let w = grid[0].len();
    let mut hits = vec![0u8; grid.len() * w];
    let mut touched: Vec<(usize, usize)> = Vec::new();
    for (t, line) in prog.iter().enumerate() {
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
            let (dy, dx) = DIRS[e];
            loop {
                y += dy;
                x += dx;
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
        for &(y, x) in &touched {
            let k = y * w + x;
            match hits[k] {
                2 => grid[y][x] = b'M',
                3.. => grid[y][x] = b'O',
                _ => {}
            }
            hits[k] = 0;
        }
        if cells.is_empty() || !cells.iter().all(|&(y, x)| get(&grid, y, x) == b'M') {
            continue;
        }
        let clean = cells.iter().all(|&(y, x)| {
            NEIGHBORS
                .iter()
                .all(|&(dy, dx)| get(&grid, y + dy, x + dx) != b'M' || in_pattern(y + dy, x + dx))
        });
        if clean {
            for &(y, x) in &cells {
                grid[y as usize][x as usize] = b'O';
            }
            subs.push(t);
            if subs.len() >= limit {
                break;
            }
        }
    }
    subs
}

/// Local search on a finished program: delete lines or merge neighbouring lines
/// as long as the program still submits the pattern `k` times.
fn polish(r: usize, pattern: &[Vec<u8>], k: u32, mut prog: Vec<[u8; 6]>) -> Vec<[u8; 6]> {
    let k = k as usize;
    let fits = |p: &[[u8; 6]]| -> Option<usize> {
        let s = submissions(r, pattern, p, k);
        (s.len() == k).then(|| s[k - 1] + 1)
    };
    let mut improved = true;
    while improved {
        improved = false;
        let mut t = 0;
        while t < prog.len() {
            // try deleting line t
            let mut cand = prog.clone();
            cand.remove(t);
            if let Some(len) = fits(&cand) {
                cand.truncate(len);
                prog = cand;
                improved = true;
                continue;
            }
            // try merging line t with line t+1 (disjoint emitters)
            if t + 1 < prog.len() && (0..6).all(|e| prog[t][e] == b'-' || prog[t + 1][e] == b'-') {
                let mut cand = prog.clone();
                let next = cand.remove(t + 1);
                for e in 0..6 {
                    if next[e] != b'-' {
                        cand[t][e] = next[e];
                    }
                }
                if let Some(len) = fits(&cand) {
                    cand.truncate(len);
                    prog = cand;
                    improved = true;
                    continue;
                }
            }
            t += 1;
        }
    }
    prog
}

fn parse_programs(text: &str) -> Vec<Vec<[u8; 6]>> {
    text.trim_matches('\n')
        .split("\n\n")
        .map(|b| {
            b.lines()
                .map(|l| {
                    let t: Vec<u8> = l.split_whitespace().map(|x| x.as_bytes()[0]).collect();
                    std::array::from_fn(|e| t[e])
                })
                .collect()
        })
        .collect()
}

/// Exact search for a building cycle of length <= 2 (line-space enumeration).
/// Returns (length, configs) where a config gives per emitter `None` (off) or a ring position.
fn min_cycle(r: usize, pattern: &[Vec<u8>]) -> Option<(usize, Vec<[Option<u8>; 6]>)> {
    if std::env::var("NOCYCLE").is_ok() {
        return None;
    }
    let g = geometry(r, pattern);
    let np = g.pat.len();
    if np > 64 || g.nf > 128 {
        return None;
    }
    let pmask: u64 = if np == 64 { u64::MAX } else { (1u64 << np) - 1 };
    let fmask: u128 = if g.nf == 128 { u128::MAX } else { (1u128 << g.nf) - 1 };
    // useful ring positions per emitter and their hit masks
    let mut useful: Vec<Vec<u8>> = vec![Vec::new(); 6];
    let mut pm: Vec<Vec<u64>> = vec![Vec::new(); 6];
    let mut fm: Vec<Vec<u128>> = vec![Vec::new(); 6];
    for e in 0..6 {
        for p in 0..g.n {
            if g.ray[e][p].is_empty() {
                continue;
            }
            let (mut a, mut b) = (0u64, 0u128);
            for &c in &g.ray[e][p] {
                let i = g.pat_idx[c as usize];
                if i >= 0 {
                    a |= 1 << i;
                }
                let f = g.f_idx[c as usize];
                if f >= 0 {
                    b |= 1 << f;
                }
            }
            useful[e].push(p as u8);
            pm[e].push(a);
            fm[e].push(b);
        }
    }
    #[derive(Clone, Copy, Default)]
    struct Planes {
        p1: u64,
        p2: u64,
        p3: u64,
        f1: u128,
        f2: u128,
        f3: u128,
    }
    fn add(pl: Planes, a: u64, b: u128) -> Planes {
        Planes {
            p3: pl.p3 | (pl.p2 & a),
            p2: (pl.p2 & !a) | (pl.p1 & a),
            p1: (pl.p1 & !a) | (!(pl.p1 | pl.p2 | pl.p3) & a),
            f3: pl.f3 | (pl.f2 & b),
            f2: (pl.f2 & !b) | (pl.f1 & b),
            f1: (pl.f1 & !b) | (!(pl.f1 | pl.f2 | pl.f3) & b),
        }
    }
    // value per emitter: 0 = off, v>0 = useful[e][v-1]
    let sizes: Vec<usize> = (0..6).map(|e| useful[e].len() + 1).collect();
    let to_cfg = |vals: &[usize; 6]| -> [Option<u8>; 6] {
        std::array::from_fn(|e| if vals[e] == 0 { None } else { Some(useful[e][vals[e] - 1]) })
    };
    // enumerate C2
    let mut vals = [0usize; 6];
    let mut best1: Option<[usize; 6]> = None;
    let mut best2: Option<([usize; 6], [usize; 6])> = None;
    fn rec(
        e: usize,
        pl: Planes,
        vals: &mut [usize; 6],
        sizes: &[usize],
        pm: &[Vec<u64>],
        fm: &[Vec<u128>],
        visit: &mut dyn FnMut(&[usize; 6], Planes) -> bool,
    ) -> bool {
        if e == 6 {
            return visit(vals, pl);
        }
        for v in 0..sizes[e] {
            vals[e] = v;
            let npl = if v == 0 { pl } else { add(pl, pm[e][v - 1], fm[e][v - 1]) };
            if npl.p3 & !0 != 0 && false {
                continue;
            }
            if rec(e + 1, npl, vals, sizes, pm, fm, visit) {
                return true;
            }
        }
        false
    }
    let mut c2_list: Vec<([usize; 6], u64, u64)> = Vec::new(); // (vals, created P, triple P|F irrelevant)
    let mut c2_triple_f: Vec<u128> = Vec::new();
    {
        let mut visit = |v: &[usize; 6], pl: Planes| -> bool {
            if pl.f2 & fmask != 0 || pl.p3 & pmask != 0 {
                return false;
            }
            if pl.p2 & pmask == pmask {
                best1 = Some(*v);
                return true;
            }
            let rem = pmask & !pl.p2;
            if rem.count_ones() <= 12 {
                c2_list.push((*v, pl.p2 & pmask, rem));
                c2_triple_f.push(pl.f3);
            }
            false
        };
        rec(0, Planes::default(), &mut vals, &sizes, &pm, &fm, &mut visit);
    }
    if let Some(v) = best1 {
        return Some((1, vec![to_cfg(&v)]));
    }
    // L = 2: C1 adjacent to C2 covers the rest; its forbidden matter must be destroyed by C2
    c2_list.sort_by_key(|c| c.2.count_ones());
    'outer: for (ci, &(v2, _, rem)) in c2_list.iter().enumerate() {
        let f3 = c2_triple_f[ci];
        // options for C1 per emitter
        let opts: Vec<Vec<usize>> = (0..6)
            .map(|e| {
                if v2[e] == 0 {
                    (0..sizes[e]).collect()
                } else {
                    let mut o = vec![0, v2[e]];
                    if v2[e] > 1 {
                        o.push(v2[e] - 1);
                    }
                    if v2[e] + 1 < sizes[e] {
                        o.push(v2[e] + 1);
                    }
                    o
                }
            })
            .collect();
        let mut v1 = [0usize; 6];
        fn rec1(
            e: usize,
            pl: Planes,
            v1: &mut [usize; 6],
            opts: &[Vec<usize>],
            pm: &[Vec<u64>],
            fm: &[Vec<u128>],
            rem: u64,
            f3: u128,
            fmask: u128,
        ) -> bool {
            if pl.p3 & rem != 0 {
                return false;
            }
            if e == 6 {
                return pl.p2 & rem == rem && (pl.f2 & fmask) & !f3 == 0;
            }
            for &v in &opts[e] {
                v1[e] = v;
                let npl = if v == 0 { pl } else { add(pl, pm[e][v - 1], fm[e][v - 1]) };
                if rec1(e + 1, npl, v1, opts, pm, fm, rem, f3, fmask) {
                    return true;
                }
            }
            false
        }
        if rec1(0, Planes::default(), &mut v1, &opts, &pm, &fm, rem, f3, fmask) {
            best2 = Some((v1, v2));
            break 'outer;
        }
    }
    best2.map(|(a, b)| (2, vec![to_cfg(&a), to_cfg(&b)]))
}

/// Program for a building cycle: move emitters into place (inactive), then repeat
/// the cycle C1 -> C2 -> C1 -> ... ; every completed cycle submits one copy.
fn cycle_program(r: usize, cfgs: &[[Option<u8>; 6]], k: u32) -> Vec<[u8; 6]> {
    let n = 6 * r;
    let h = home(r);
    let first_on = |e: usize| cfgs.iter().find_map(|c| c[e]);
    let step_dir = |from: usize, to: usize| -> u8 {
        if (from + 1) % n == to {
            b'F'
        } else if (to + 1) % n == from {
            b'B'
        } else {
            b'-'
        }
    };
    // setup moves
    let mut moves: Vec<Vec<u8>> = vec![Vec::new(); 6];
    for e in 0..6 {
        if let Some(t) = first_on(e) {
            let t = t as usize;
            let fwd = (t + n - h[e]) % n;
            if fwd <= n - fwd {
                moves[e] = vec![b'F'; fwd];
            } else {
                moves[e] = vec![b'B'; n - fwd];
            }
        }
    }
    let d = moves.iter().map(Vec::len).max().unwrap_or(0);
    let mut prog: Vec<[u8; 6]> = (0..d).map(|i| std::array::from_fn(|e| *moves[e].get(i).unwrap_or(&b'-'))).collect();
    // activation line = first config of the cycle
    let c1 = cfgs[0];
    prog.push(std::array::from_fn(|e| if c1[e].is_some() { b'T' } else { b'-' }));
    let trans = |a: &[Option<u8>; 6], b: &[Option<u8>; 6]| -> [u8; 6] {
        std::array::from_fn(|e| match (a[e], b[e]) {
            (Some(x), Some(y)) => step_dir(x as usize, y as usize),
            (None, None) => b'-',
            _ => b'T',
        })
    };
    if cfgs.len() == 1 {
        for _ in 1..k {
            prog.push([b'-'; 6]);
        }
    } else {
        let fwd = trans(&cfgs[0], &cfgs[1]);
        let back = trans(&cfgs[1], &cfgs[0]);
        prog.push(fwd);
        for _ in 1..k {
            prog.push(back);
            prog.push(fwd);
        }
    }
    prog
}

/// Simulate a program prefix; returns (search state, submissions so far) or None if
/// forbidden cells hold matter (the beam search cannot continue from such states).
fn state_after(r: usize, _pattern: &[Vec<u8>], g: &Geometry, prog: &[[u8; 6]]) -> Option<(St, usize)> {
    let mut grid = empty_board(r);
    let ring = ring(r);
    let n = ring.len();
    let w = grid[0].len();
    let mut idx = home(r);
    let mut active = [false; 6];
    let mut subs = 0;
    let cells: Vec<usize> = g.pat.iter().map(|&c| c as usize).collect();
    for line in prog {
        for e in 0..6 {
            match line[e] {
                b'F' => idx[e] = (idx[e] + 1) % n,
                b'B' => idx[e] = (idx[e] + n - 1) % n,
                b'T' => active[e] = !active[e],
                _ => {}
            }
        }
        let mut hits = vec![0u8; grid.len() * w];
        for e in 0..6 {
            if active[e] {
                for &c in &g.ray[e][idx[e]] {
                    hits[c as usize] += 1;
                }
            }
        }
        for (c, &h) in hits.iter().enumerate() {
            match h {
                2 => grid[c / w][c % w] = b'M',
                3.. => grid[c / w][c % w] = b'O',
                _ => {}
            }
        }
        let full = cells.iter().all(|&c| grid[c / w][c % w] == b'M');
        let clean = (0..g.ncell).all(|c| !g.forbidden[c] || grid[c / w][c % w] != b'M');
        if full && clean {
            for &c in &cells {
                grid[c / w][c % w] = b'O';
            }
            subs += 1;
        }
    }
    if (0..g.ncell).any(|c| g.forbidden[c] && grid[c / w][c % w] == b'M') {
        return None;
    }
    let mut have = [0u64; NW];
    for (i, &c) in cells.iter().enumerate() {
        if grid[c / w][c % w] == b'M' {
            have[i / 64] |= 1 << (i % 64);
        }
    }
    let act = (0..6).fold(0u8, |a, e| a | ((active[e] as u8) << e));
    Some((St { pos: std::array::from_fn(|e| idx[e] as u8), act, have, junk: [0; NJ] }, subs))
}

/// Re-plan the tail of a program from several cut points; keeps the shortest valid result.
fn reoptimize(r: usize, pattern: &[Vec<u8>], k: u32, prog: Vec<[u8; 6]>, width: usize) -> Vec<[u8; 6]> {
    let g = geometry(r, pattern);
    if g.pat.len() > NW * 64 {
        return prog;
    }
    let step = env_num("CUTSTEP", 1).max(1) as usize;
    let mut best = prog.clone();
    let mut t = 0;
    while t < prog.len() {
        if let Some((st, subs)) = state_after(r, pattern, &g, &prog[..t]) {
            if subs < k as usize {
                if let Some(tail) = plan_beam_from(&g, r, k - subs as u32, width, Some(st)) {
                    if t + tail.len() < best.len() {
                        let mut cand = prog[..t].to_vec();
                        cand.extend(tail);
                        if simulate(r, pattern, &cand) == k {
                            best = cand;
                        }
                    }
                }
            }
        }
        t += step;
    }
    best
}

/// Re-plan program tails with the whole-program beam from several cut points.
fn reopt_ext(r: usize, pattern: &[Vec<u8>], k: u32, prog: Vec<[u8; 6]>, width: usize) -> Vec<[u8; 6]> {
    let g = geometry(r, pattern);
    let params = extbeam::Params {
        width,
        wa: env_num("WA", 100),
        wd: env_num("WD", 30),
        noise: env_num("NZ", 0) as u64,
        seed: env_num("SEED", 0) as u64,
        maxt: env_num("MAXT", 600) as usize,
    };
    let step = env_num("CUTSTEP", 2).max(1) as usize;
    let mut best = prog.clone();
    let mut t = prog.len().saturating_sub(1);
    loop {
        if let Some(cand) = extbeam::plan_from(&g, r, k, &params, &prog[..t]) {
            if cand.len() < best.len() && simulate(r, pattern, &cand) == k {
                best = cand;
            }
        }
        if t < step {
            break;
        }
        t -= step;
    }
    best
}

fn generate(r: usize, pattern: &[Vec<u8>], k: u32, width: usize) -> Vec<[u8; 6]> {
    let g = geometry(r, pattern);
    let mut best: Option<Vec<[u8; 6]>> = None;
    for a in 0..6 {
        for b in a + 1..6 {
            if let Some(p) = plan(&g, r, a, b, k) {
                if best.as_ref().map_or(true, |x| p.len() < x.len()) {
                    best = Some(p);
                }
            }
        }
    }
    let only_ext = std::env::var("ONLYEXT").is_ok();
    if width > 0 && !only_ext && g.pat.len() <= 30 && r <= 6 {
        if let Some((_, cfgs)) = min_cycle(r, pattern) {
            let p = cycle_program(r, &cfgs, k);
            if simulate(r, pattern, &p) == k && best.as_ref().map_or(true, |x| p.len() < x.len()) {
                best = Some(p);
            }
        }
    }
    if width > 0 && !only_ext {
        if let Some(p) = plan_beam(&g, r, k, width) {
            if simulate(r, pattern, &p) == k && best.as_ref().map_or(true, |x| p.len() < x.len()) {
                best = Some(p);
            }
        }
    }
    // SEG=0 disables the segment search (free start, forward/backward walks)
    if width > 0 && !only_ext && k >= 2 && env_num("SEG", 1) != 0 {
        if let Some(p) = segment::segment_program(&g, r, pattern, k, width) {
            if simulate(r, pattern, &p) == k && best.as_ref().map_or(true, |x| p.len() < x.len()) {
                best = Some(p);
            }
        }
        if let Some(p) = best.as_ref().and_then(|b| segment::reuse_best_copy(r, pattern, k, b)) {
            best = Some(p);
        }
    }
    let extw = env_num("EXTW", 0) as usize;
    if extw > 0 && width > 0 {
        let params = extbeam::Params {
            width: extw,
            wa: env_num("WA", 100),
            wd: env_num("WD", 30),
            noise: env_num("NZ", 0) as u64,
            seed: env_num("SEED", 0) as u64,
            maxt: env_num("MAXT", 600) as usize,
        };
        if let Some(p) = extbeam::plan(&g, r, k, &params) {
            if simulate(r, pattern, &p) == k && best.as_ref().map_or(true, |x| p.len() < x.len()) {
                best = Some(p);
            }
        }
    }
    best.expect("no program found")
}

struct Task {
    r: usize,
    pattern: Vec<Vec<u8>>,
    k: u32,
}

fn parse(input: &str) -> Vec<Task> {
    let mut t = input.split_whitespace();
    let n: usize = t.next().unwrap().parse().unwrap();
    (0..n)
        .map(|_| {
            let r: usize = t.next().unwrap().parse().unwrap();
            let rows: usize = t.next().unwrap().parse().unwrap();
            let _cols = t.next().unwrap();
            let k: u32 = t.next().unwrap().parse().unwrap();
            let pattern = (0..rows).map(|_| t.next().unwrap().bytes().collect()).collect();
            Task { r, pattern, k }
        })
        .collect()
}

fn solve(input: &str, existing: Option<Vec<Vec<[u8; 6]>>>) -> (String, usize) {
    let tasks = parse(input);
    let polish_only = std::env::var("POLISH").is_ok();
    if std::env::var("VERIFY").is_ok() {
        let ex = existing.expect("keine Programme zum Prüfen");
        let bad: Vec<usize> = tasks
            .iter()
            .enumerate()
            .filter(|(i, t)| ex.get(*i).map_or(true, |p| simulate(t.r, &t.pattern, p) != t.k))
            .map(|(i, _)| i)
            .collect();
        let total = ex.iter().map(Vec::len).sum();
        println!("ungültig: {} {:?}", bad.len(), &bad[..bad.len().min(10)]);
        return (String::new(), total);
    }
    let width = env_num("BEAM", 16) as usize;
    // SHARD=i/n: only tasks with index % n == i get the expensive beam search
    let (shard, nshard) = std::env::var("SHARD")
        .ok()
        .and_then(|v| v.split_once('/').map(|(a, b)| (a.parse().unwrap_or(0), b.parse().unwrap_or(1))))
        .unwrap_or((0usize, 1usize));
    let limit = env_num("LIMIT", i64::MAX) as usize;
    let next = std::sync::atomic::AtomicUsize::new(0);
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get());
    let mut progs: Vec<(usize, Vec<[u8; 6]>)> = std::thread::scope(|s| {
        let handles: Vec<_> = (0..threads)
            .map(|_| {
                s.spawn(|| {
                    let mut done = Vec::new();
                    loop {
                        let i = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        let Some(t) = tasks.get(i) else { break };
                        let w = if i % nshard == shard && i < limit { width } else { 0 };
                        let old = existing.as_ref().and_then(|e| e.get(i)).filter(|p| simulate(t.r, &t.pattern, p) == t.k);
                        let mut p = match (polish_only, old) {
                            (true, Some(o)) => o.clone(),
                            (_, o) => {
                                let g = generate(t.r, &t.pattern, t.k, w);
                                match o {
                                    Some(o) if o.len() < g.len() => o.clone(),
                                    _ => g,
                                }
                            }
                        };
                        let rew = env_num("REOPT2", 0) as usize;
                        if w > 0 && rew > 0 {
                            p = reopt_ext(t.r, &t.pattern, t.k, p, rew);
                        }
                        if w > 0 && std::env::var("REOPT").is_ok() {
                            p = reoptimize(t.r, &t.pattern, t.k, p, w);
                        }
                        if w > 0 || polish_only {
                            p = polish(t.r, &t.pattern, t.k, p);
                        }
                        if polish_only {
                            if let Some(q) = segment::reuse_best_copy(t.r, &t.pattern, t.k, &p) {
                                if q.len() < p.len() && simulate(t.r, &t.pattern, &q) == t.k {
                                    p = q;
                                }
                            }
                        }
                        assert_eq!(simulate(t.r, &t.pattern, &p), t.k, "verification failed");
                        done.push((i, p));
                    }
                    done
                })
            })
            .collect();
        handles.into_iter().flat_map(|h| h.join().unwrap()).collect()
    });
    progs.sort_by_key(|p| p.0);
    let total = progs.iter().map(|p| p.1.len()).sum();
    let mut out = String::new();
    for (_, p) in progs {
        for line in p {
            let toks: Vec<String> = line.iter().map(|&c| (c as char).to_string()).collect();
            out += &toks.join(" ");
            out.push('\n');
        }
        out.push('\n');
    }
    (out, total)
}

fn main() {
    if std::env::var("ANALYZE2").is_ok() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("daten");
        let f = std::env::var("ANALYZE2").unwrap();
        let tasks = parse(&fs::read_to_string(dir.join(f)).unwrap());
        for t in tasks.iter().take(env_num("LIMIT", 12) as usize) {
            let g = geometry(t.r, &t.pattern);
            let hist = endgame::config_histogram(&g);
            println!("R={} np={} k={} max={} hist(>=6..)={:?}", t.r, g.pat.len(), t.k, hist.len() - 1, &hist[6.min(hist.len())..]);
        }
        return;
    }
    if std::env::var("ANALYZE").is_ok() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("daten");
        for f in ["in_level-6_2-medium.txt"] {
            let tasks = parse(&fs::read_to_string(dir.join(f)).unwrap());
            let (mut l1, mut l2, mut none, mut copies2) = (0, 0, 0, 0);
            for t in tasks.iter().take(0) {
                match min_cycle(t.r, &t.pattern) {
                    Some((1, _)) => l1 += 1,
                    Some((2, _)) => {
                        l2 += 1;
                        copies2 += t.k;
                    }
                    _ => none += 1,
                }
            }
            println!("{f}: L=1: {l1}, L=2: {l2} (Kopien {copies2}), mehr: {none}");
            let old = parse_programs(&fs::read_to_string(dir.join(f.replace("in_", "out_"))).unwrap());
            let res: Vec<Option<usize>> = std::thread::scope(|sc| {
                let hs: Vec<_> = tasks
                    .chunks(tasks.len().div_ceil(4))
                    .map(|c| sc.spawn(move || c.iter().map(|t| min_cycle(t.r, &t.pattern).map(|x| x.0)).collect::<Vec<_>>()))
                    .collect();
                hs.into_iter().flat_map(|h| h.join().unwrap()).collect()
            });
            let mut agg = std::collections::BTreeMap::new();
            for (i, t) in tasks.iter().enumerate() {
                let key = res[i].unwrap_or(3);
                let e = agg.entry(key).or_insert((0usize, 0usize, 0usize));
                e.0 += 1;
                e.1 += t.k as usize;
                e.2 += old[i].len();
            }
            for (l, (cnt, copies, lines)) in agg {
                println!("  L{}{}: Muster {cnt}, Kopien {copies}, Zeilen bisher {lines} ({:.2} pro Kopie)", if l == 3 { ">=" } else { "=" }, l, lines as f64 / copies as f64);
            }
        }
        return;
    }
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("daten");
    // OUT: output directory (default: daten), FILES: comma separated name filters
    let out_dir = std::env::var("OUT").map(std::path::PathBuf::from).unwrap_or(dir.clone());
    fs::create_dir_all(&out_dir).unwrap();
    let filters: Vec<String> = std::env::var("FILES").map(|v| v.split(',').map(String::from).collect()).unwrap_or_default();
    let mut files: Vec<_> = fs::read_dir(&dir).unwrap().flatten().map(|e| e.path()).collect();
    files.sort();
    for p in files {
        let name = p.file_name().unwrap().to_string_lossy().into_owned();
        if !filters.is_empty() && !filters.iter().any(|f| name.contains(f.as_str())) {
            continue;
        }
        if let Some(rest) = name.strip_prefix("in_") {
            let existing = fs::read_to_string(out_dir.join(format!("out_{rest}"))).ok().map(|t| parse_programs(&t));
            let (res, total) = solve(&fs::read_to_string(&p).unwrap(), existing);
            if std::env::var("VERIFY").is_err() {
                fs::write(out_dir.join(format!("out_{rest}")), res).unwrap();
            }
            println!("{name}: ok, Zeilen gesamt = {total}");
            endgame::report();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn beispiel_programme_sind_gueltig() {
        let d = Path::new(env!("CARGO_MANIFEST_DIR")).join("daten");
        let inp = fs::read_to_string(d.join("in_level-6_0-example.txt")).unwrap();
        let (_, total) = solve(&inp, None);
        assert!(total > 0);
    }
}
