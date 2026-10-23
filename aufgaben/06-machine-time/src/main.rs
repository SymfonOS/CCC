use std::{fs, path::Path};

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
    forbidden: Vec<bool>,    // non-pattern cells adjacent to the pattern
    ncell: usize,
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
    Geometry { n, ray, pat, pat_idx, forbidden, ncell }
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

fn generate(r: usize, pattern: &[Vec<u8>], k: u32) -> Vec<[u8; 6]> {
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

fn solve(input: &str) -> (String, usize) {
    let tasks = parse(input);
    let progs: Vec<Vec<[u8; 6]>> = std::thread::scope(|s| {
        let threads = 8;
        let chunk = tasks.len().div_ceil(threads).max(1);
        let handles: Vec<_> = tasks
            .chunks(chunk)
            .map(|c| {
                s.spawn(move || {
                    c.iter()
                        .map(|t| {
                            let p = generate(t.r, &t.pattern, t.k);
                            assert_eq!(simulate(t.r, &t.pattern, &p), t.k, "verification failed");
                            p
                        })
                        .collect::<Vec<_>>()
                })
            })
            .collect();
        handles.into_iter().flat_map(|h| h.join().unwrap()).collect()
    });
    let total = progs.iter().map(Vec::len).sum();
    let mut out = String::new();
    for p in progs {
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
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("daten");
    let mut files: Vec<_> = fs::read_dir(&dir).unwrap().flatten().map(|e| e.path()).collect();
    files.sort();
    for p in files {
        let name = p.file_name().unwrap().to_string_lossy().into_owned();
        if let Some(rest) = name.strip_prefix("in_") {
            let (res, total) = solve(&fs::read_to_string(&p).unwrap());
            fs::write(dir.join(format!("out_{rest}")), res).unwrap();
            println!("{name}: ok, Zeilen gesamt = {total}");
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
        let (_, total) = solve(&inp);
        assert!(total > 0);
    }
}
