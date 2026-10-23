//! Beam search over all copies at once (one beam for the whole program).
//! Score per state: subs * 1e7 + painted * WA - sd * WD (+ noise), where `sd` is the sum over
//! emitters of the distance to the nearest line that still contains an unpainted pattern cell.

use crate::{home, Geometry, INSTR};

const NWM: usize = 8;

#[derive(Clone, Copy)]
struct S2 {
    m: [u64; NWM], // matter on relevant cells (pattern + forbidden)
    o: [u8; 6],    // offset index 0..=2R (offset - R)
    act: u8,
    subs: u8,
}

#[derive(Clone, Copy)]
struct Ch {
    score: i64,
    h: u64,
    par: u32,
    code: u16,
}

pub struct Params {
    pub width: usize,
    pub wa: i64,
    pub wd: i64,
    pub noise: u64,
    pub seed: u64,
    pub maxt: usize,
}

fn compact(cand: &mut Vec<Ch>, width: usize, hs: &mut Vec<u64>) -> i64 {
    cand.sort_unstable_by(|a, b| b.score.cmp(&a.score).then(a.h.cmp(&b.h)));
    hs.iter_mut().for_each(|x| *x = 0);
    let n = hs.len();
    let mut k = 0;
    for i in 0..cand.len() {
        if k >= width {
            break;
        }
        let h = cand[i].h | 1;
        let mut j = (h % n as u64) as usize;
        let mut dup = false;
        while hs[j] != 0 {
            if hs[j] == h {
                dup = true;
                break;
            }
            j = (j + 1) % n;
        }
        if dup {
            continue;
        }
        hs[j] = h;
        cand[k] = cand[i];
        k += 1;
    }
    cand.truncate(k);
    if k == width {
        cand[k - 1].score
    } else {
        i64::MIN
    }
}

pub fn plan(g: &Geometry, r: usize, k: u32, p: &Params) -> Option<Vec<[u8; 6]>> {
    // relevant cells: pattern + forbidden
    let mut rel = vec![usize::MAX; g.ncell];
    let mut nrel = 0;
    let mut pat = [0u64; NWM];
    let mut halo = [0u64; NWM];
    for c in 0..g.ncell {
        let is_p = g.pat_idx[c] >= 0;
        if is_p || g.forbidden[c] {
            if nrel >= NWM * 64 {
                return None;
            }
            rel[c] = nrel;
            if is_p {
                pat[nrel / 64] |= 1 << (nrel % 64);
            } else {
                halo[nrel / 64] |= 1 << (nrel % 64);
            }
            nrel += 1;
        }
    }
    let nw = nrel.div_ceil(64).max(1);
    let no = 2 * r + 1;
    let n = g.n;
    let h = home(r);
    // beam masks per emitter and offset index
    let mut bm = vec![[[0u64; NWM]; 32]; 6];
    for e in 0..6 {
        for oi in 0..no {
            let pos = (h[e] + n + oi - r) % n;
            for &c in &g.ray[e][pos] {
                let i = rel[c as usize];
                if i != usize::MAX {
                    bm[e][oi][i / 64] |= 1 << (i % 64);
                }
            }
        }
    }
    let mut rng: u64 = p.seed.wrapping_mul(0x9E3779B97F4A7C15) ^ 0x2545F4914F6CDD1D ^ (g.pat.len() as u64);
    let mut xr = move || {
        rng ^= rng << 13;
        rng ^= rng >> 7;
        rng ^= rng << 17;
        rng
    };
    let mut cur: Vec<S2> = vec![S2 { m: [0; NWM], o: [r as u8; 6], act: 0, subs: 0 }];
    let cap = p.width * 8 + 4096;
    let mut cand: Vec<Ch> = Vec::with_capacity(cap);
    let mut hs = vec![0u64; p.width * 4 + 7];
    let mut hist: Vec<Vec<(u32, u16)>> = Vec::new();
    let mut found: Option<(u32, u16)> = None;
    for _t in 0..p.maxt {
        cand.clear();
        let mut thr = i64::MIN;
        'parents: for (pi, s) in cur.iter().enumerate() {
            let mut un = [0u64; NWM];
            for w in 0..nw {
                un[w] = pat[w] & !s.m[w];
            }
            // distance tables
            let mut dt = [[0i64; 32]; 6];
            let mut need = [false; 6];
            for e in 0..6 {
                let mut has = [false; 32];
                for oi in 0..no {
                    has[oi] = (0..nw).any(|w| bm[e][oi][w] & un[w] != 0);
                    need[e] |= has[oi];
                }
                if need[e] {
                    for oi in 0..no {
                        let mut d = 99i64;
                        for o2 in 0..no {
                            if has[o2] {
                                d = d.min((o2 as i64 - oi as i64).abs());
                            }
                        }
                        dt[e][oi] = d;
                    }
                }
            }
            // options per emitter: (offset index, active, code)
            let mut opts = [[(0u8, false, 0u16); 4]; 6];
            let mut nopt = [0usize; 6];
            for e in 0..6 {
                let o = s.o[e];
                let a = (s.act >> e) & 1 != 0;
                let mut c = 0;
                opts[e][c] = (o, a, 0);
                c += 1;
                if (o as usize) + 1 < no {
                    opts[e][c] = (o + 1, a, 1);
                    c += 1;
                }
                if o > 0 {
                    opts[e][c] = (o - 1, a, 2);
                    c += 1;
                }
                opts[e][c] = (o, !a, 3);
                c += 1;
                nopt[e] = c;
            }
            // DFS over emitters with incremental hit planes
            let mut c1 = [[0u64; NWM]; 7];
            let mut c2 = [[0u64; NWM]; 7];
            let mut c3 = [[0u64; NWM]; 7];
            let mut idx = [usize::MAX; 6];
            let mut lvl: isize = 0;
            while lvl >= 0 {
                let l = lvl as usize;
                idx[l] = idx[l].wrapping_add(1);
                if idx[l] >= nopt[l] {
                    idx[l] = usize::MAX;
                    lvl -= 1;
                    continue;
                }
                let (oi, a, _) = opts[l][idx[l]];
                if a {
                    let b = &bm[l][oi as usize];
                    let mut bad = false;
                    for w in 0..nw {
                        let x = b[w];
                        c3[l + 1][w] = c3[l][w] | (c2[l][w] & x);
                        c2[l + 1][w] = c2[l][w] | (c1[l][w] & x);
                        c1[l + 1][w] = c1[l][w] | x;
                        if c3[l + 1][w] & s.m[w] & pat[w] != 0 {
                            bad = true;
                        }
                    }
                    if bad {
                        continue;
                    }
                } else {
                    c1[l + 1] = c1[l];
                    c2[l + 1] = c2[l];
                    c3[l + 1] = c3[l];
                }
                if l < 5 {
                    lvl += 1;
                    continue;
                }
                // leaf
                let mut nm = [0u64; NWM];
                let mut bad = false;
                for w in 0..nw {
                    let ex2 = c2[6][w] & !c3[6][w];
                    if ex2 & halo[w] != 0 {
                        bad = true;
                        break;
                    }
                    nm[w] = (s.m[w] | ex2) & !c3[6][w];
                }
                if bad {
                    continue;
                }
                let full = (0..nw).all(|w| nm[w] & pat[w] == pat[w] && nm[w] & halo[w] == 0);
                let mut subs = s.subs as u32;
                if full {
                    subs += 1;
                    for w in 0..nw {
                        nm[w] &= !pat[w];
                    }
                }
                let mut code: u16 = 0;
                let mut sd = 0i64;
                let mut hsh: u64 = 1469598103934665603;
                for e in 0..6 {
                    let (oi, a, c) = opts[e][idx[e]];
                    code |= c << (2 * e);
                    sd += dt[e][oi as usize] + if need[e] && !a { 1 } else { 0 };
                    hsh ^= oi as u64 | ((a as u64) << 8);
                    hsh = hsh.wrapping_mul(1099511628211);
                }
                if subs >= k {
                    found = Some((pi as u32, code));
                    break 'parents;
                }
                let painted: i64 = (0..nw).map(|w| (nm[w] & pat[w]).count_ones() as i64).sum();
                let mut sc = subs as i64 * 10_000_000 + painted * p.wa - sd * p.wd;
                if p.noise > 0 {
                    sc += (xr() % p.noise) as i64;
                }
                if sc < thr {
                    continue;
                }
                for w in 0..nw {
                    hsh ^= nm[w];
                    hsh = hsh.wrapping_mul(1099511628211);
                    hsh ^= hsh >> 29;
                }
                hsh ^= subs as u64;
                hsh = hsh.wrapping_mul(1099511628211);
                cand.push(Ch { score: sc, h: hsh, par: pi as u32, code });
                if cand.len() >= cap {
                    thr = compact(&mut cand, p.width, &mut hs);
                }
            }
        }
        if let Some(f) = found {
            hist.push(vec![f]);
            break;
        }
        compact(&mut cand, p.width, &mut hs);
        if cand.is_empty() {
            return None;
        }
        let mut next = Vec::with_capacity(cand.len());
        let mut layer = Vec::with_capacity(cand.len());
        for c in &cand {
            let mut d = cur[c.par as usize];
            let (mut p1, mut p2, mut p3) = ([0u64; NWM], [0u64; NWM], [0u64; NWM]);
            for e in 0..6 {
                match (c.code >> (2 * e)) & 3 {
                    1 => d.o[e] += 1,
                    2 => d.o[e] -= 1,
                    3 => d.act ^= 1 << e,
                    _ => {}
                }
                if (d.act >> e) & 1 != 0 {
                    let b = &bm[e][d.o[e] as usize];
                    for w in 0..nw {
                        p3[w] |= p2[w] & b[w];
                        p2[w] |= p1[w] & b[w];
                        p1[w] |= b[w];
                    }
                }
            }
            let mut full = true;
            for w in 0..nw {
                d.m[w] = (d.m[w] | (p2[w] & !p3[w])) & !p3[w];
                if d.m[w] & pat[w] != pat[w] || d.m[w] & halo[w] != 0 {
                    full = false;
                }
            }
            if full {
                d.subs += 1;
                for w in 0..nw {
                    d.m[w] &= !pat[w];
                }
            }
            next.push(d);
            layer.push((c.par, c.code));
        }
        hist.push(layer);
        cur = next;
    }
    found?;
    // backtrack
    let t = hist.len() - 1;
    let mut codes = vec![0u16; t + 1];
    let (mut par, code) = hist[t][0];
    codes[t] = code;
    for tt in (0..t).rev() {
        let (pp, cc) = hist[tt][par as usize];
        codes[tt] = cc;
        par = pp;
    }
    let sym = [b'-', b'F', b'B', b'T'];
    let _ = INSTR;
    Some(
        codes
            .iter()
            .map(|&c| std::array::from_fn(|e| sym[((c >> (2 * e)) & 3) as usize]))
            .collect(),
    )
}
