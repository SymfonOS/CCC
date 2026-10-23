//! Endgame support for the beam planner (level 6).

use std::sync::atomic::{AtomicUsize, Ordering};

static TICKS: AtomicUsize = AtomicUsize::new(0);
static TAIL: AtomicUsize = AtomicUsize::new(0);

/// Record one finished copy: `have_per_tick[t]` = pattern cells present after tick t (best state).
pub fn record_copy(have_per_tick: &[usize], np: usize) {
    let t80 = have_per_tick.iter().position(|&h| h * 5 >= np * 4).unwrap_or(have_per_tick.len());
    TICKS.fetch_add(have_per_tick.len(), Ordering::Relaxed);
    TAIL.fetch_add(have_per_tick.len().saturating_sub(t80 + 1), Ordering::Relaxed);
}

pub fn report() {
    if std::env::var("STATS").is_ok() {
        eprintln!("ticks={} tail(after 80%)={}", TICKS.load(Ordering::Relaxed), TAIL.load(Ordering::Relaxed));
    }
}

fn env_num(name: &str, default: i64) -> i64 {
    std::env::var(name).ok().and_then(|v| v.parse().ok()).unwrap_or(default)
}

thread_local! {
    static EWF: usize = env_num("EWF", 1) as usize;
    static EPCT: usize = env_num("EPCT", 80) as usize;
}

/// Beam width for the current tick: wider once most of the pattern exists.
pub fn width(base: usize, best_have: usize, np: usize) -> usize {
    let f = EWF.with(|v| *v);
    let pct = EPCT.with(|v| *v);
    if f > 1 && best_have * 100 >= np * pct {
        base * f
    } else {
        base
    }
}

thread_local! {
    static LA: usize = env_num("LA", 0) as usize;
    static LAM: usize = env_num("LAM", 3) as usize;
    static LAW: i64 = env_num("LAW", 100);
}

pub fn lookahead_factor() -> usize {
    LA.with(|v| *v)
}

pub fn lookahead_weight() -> i64 {
    LAW.with(|v| *v)
}

/// Moves used for the one-step lookahead (at most LAM acting emitters).
pub fn lookahead_moves() -> Vec<[u8; 6]> {
    let movers = LAM.with(|v| *v);
    (0..4usize.pow(6))
        .filter_map(|code| {
            let mut m = [3u8; 6];
            let mut c = code;
            for e in m.iter_mut() {
                *e = (c % 4) as u8;
                c /= 4;
            }
            (m.iter().filter(|&&x| x != 3).count() <= movers).then_some(m)
        })
        .collect()
}

/// Largest number of new pattern cells any single valid next line can create.
pub fn best_next_gain(g: &crate::Geometry, st: &crate::St, moves: &[[u8; 6]], cnt: &mut [u8], touched: &mut Vec<u16>) -> usize {
    let n = g.n;
    let mut best = 0;
    for mv in moves {
        let mut pos = st.pos;
        let mut act = st.act;
        for e in 0..6 {
            match mv[e] {
                0 => pos[e] = ((pos[e] as usize + 1) % n) as u8,
                1 => pos[e] = ((pos[e] as usize + n - 1) % n) as u8,
                2 => act ^= 1 << e,
                _ => {}
            }
        }
        if act.count_ones() < 2 {
            continue;
        }
        touched.clear();
        for e in 0..6 {
            if (act >> e) & 1 != 0 {
                for &c in &g.ray[e][pos[e] as usize] {
                    if cnt[c as usize] == 0 {
                        touched.push(c);
                    }
                    cnt[c as usize] += 1;
                }
            }
        }
        let mut ok = true;
        let mut gain = 0;
        for &c in touched.iter() {
            if cnt[c as usize] == 2 {
                if g.forbidden[c as usize] {
                    ok = false;
                }
                let i = g.pat_idx[c as usize];
                if i >= 0 && (st.have[i as usize / 64] >> (i as usize % 64)) & 1 == 0 {
                    gain += 1;
                }
            }
            cnt[c as usize] = 0;
        }
        if ok && gain > best {
            best = gain;
        }
    }
    best
}

/// Parameter variants tried for every copy (VARIANTS env, default 1 = only the base):
/// base, other potential, lookahead, both, then noisy copies of these.
pub fn variants(base: &crate::BeamParams) -> Vec<crate::BeamParams> {
    let n = env_num("VARIANTS", 1).max(1) as usize;
    (0..n)
        .map(|v| {
            let mut p = base.clone();
            if v % 2 == 1 {
                p.pfam = !base.pfam;
            }
            if (v / 2) % 2 == 1 {
                p.la = if base.la > 0 { 0 } else { 4 };
                p.la_full = true;
            }
            if v >= 4 {
                p.noise = 40;
                p.seed = base.seed.wrapping_add(v as u64 * 1_000_003);
            }
            p
        })
        .collect()
}

/// Histogram over all forbidden-free emitter configurations: number of pattern cells created.
pub fn config_histogram(g: &crate::Geometry) -> Vec<u64> {
    let mut useful: Vec<Vec<usize>> = vec![Vec::new(); 6];
    for e in 0..6 {
        for p in 0..g.n {
            if !g.ray[e][p].is_empty() {
                useful[e].push(p);
            }
        }
    }
    let mut hist = vec![0u64; 64];
    let mut cnt = vec![0u8; g.ncell];
    let mut idx = [0usize; 6];
    let sizes: Vec<usize> = (0..6).map(|e| useful[e].len() + 1).collect();
    loop {
        let mut touched: Vec<u16> = Vec::new();
        for e in 0..6 {
            if idx[e] > 0 {
                for &c in &g.ray[e][useful[e][idx[e] - 1]] {
                    if cnt[c as usize] == 0 {
                        touched.push(c);
                    }
                    cnt[c as usize] += 1;
                }
            }
        }
        let mut ok = true;
        let mut gain = 0;
        for &c in &touched {
            if cnt[c as usize] == 2 {
                if g.forbidden[c as usize] {
                    ok = false;
                }
                if g.pat_idx[c as usize] >= 0 {
                    gain += 1;
                }
            }
            cnt[c as usize] = 0;
        }
        if ok {
            hist[gain.min(63)] += 1;
        }
        let mut e = 0;
        loop {
            if e == 6 {
                while hist.len() > 1 && *hist.last().unwrap() == 0 {
                    hist.pop();
                }
                return hist;
            }
            idx[e] += 1;
            if idx[e] < sizes[e] {
                break;
            }
            idx[e] = 0;
            e += 1;
        }
    }
}

/// Forbidden-free emitter configurations (emitters on lines through the pattern), best first.
pub struct ConfigList {
    pos: Vec<[u8; 6]>,
    on: Vec<u8>,
    mask: Vec<[u64; crate::NW]>,
}

thread_local! {
    static TW: i64 = env_num("TW", 0);
    static LCAP: usize = env_num("LCAP", 50_000) as usize;
}

pub fn target_weight() -> i64 {
    TW.with(|v| *v)
}

pub fn build_configs(g: &crate::Geometry) -> ConfigList {
    let mut useful: Vec<Vec<usize>> = vec![Vec::new(); 6];
    for e in 0..6 {
        for p in 0..g.n {
            if g.ray[e][p].iter().any(|&c| g.pat_idx[c as usize] >= 0) {
                useful[e].push(p);
            }
        }
    }
    let sizes: Vec<usize> = (0..6).map(|e| useful[e].len() + 1).collect();
    let mut idx = [0usize; 6];
    let mut cnt = vec![0u8; g.ncell];
    let mut touched: Vec<u16> = Vec::new();
    let mut all: Vec<(u32, [u8; 6], u8, [u64; crate::NW])> = Vec::new();
    'enumerate: loop {
        touched.clear();
        let mut on = 0u8;
        let mut pos = [0u8; 6];
        for e in 0..6 {
            if idx[e] > 0 {
                let p = useful[e][idx[e] - 1];
                on |= 1 << e;
                pos[e] = p as u8;
                for &c in &g.ray[e][p] {
                    if cnt[c as usize] == 0 {
                        touched.push(c);
                    }
                    cnt[c as usize] += 1;
                }
            }
        }
        let mut ok = true;
        let mut mask = [0u64; crate::NW];
        let mut gain = 0u32;
        for &c in &touched {
            let v = cnt[c as usize];
            if v == 2 {
                if g.forbidden[c as usize] {
                    ok = false;
                }
                let i = g.pat_idx[c as usize];
                if i >= 0 {
                    mask[i as usize / 64] |= 1 << (i as usize % 64);
                    gain += 1;
                }
            }
            cnt[c as usize] = 0;
        }
        if ok && gain >= 2 {
            all.push((gain, pos, on, mask));
        }
        let mut e = 0;
        loop {
            if e == 6 {
                break 'enumerate;
            }
            idx[e] += 1;
            if idx[e] < sizes[e] {
                break;
            }
            idx[e] = 0;
            e += 1;
        }
    }
    let cap = LCAP.with(|v| *v);
    if all.len() > cap {
        all.select_nth_unstable_by(cap, |a, b| b.0.cmp(&a.0));
        all.truncate(cap);
    }
    all.sort_unstable_by(|a, b| b.0.cmp(&a.0));
    ConfigList {
        pos: all.iter().map(|a| a.1).collect(),
        on: all.iter().map(|a| a.2).collect(),
        mask: all.iter().map(|a| a.3).collect(),
    }
}

/// Best reachable rate (new pattern cells per tick, x100) over the configuration list.
pub fn rate_term(g: &crate::Geometry, list: &ConfigList, st: &crate::St) -> i64 {
    let n = g.n;
    let np = g.pat.len();
    let words = np.div_ceil(64);
    let mut rem = [0u64; crate::NW];
    for w in 0..words {
        let full = if (w + 1) * 64 <= np { u64::MAX } else { (1u64 << (np % 64)) - 1 };
        rem[w] = full & !st.have[w];
    }
    let mut best = 0i64;
    for i in 0..list.pos.len() {
        let m = &list.mask[i];
        let mut y = 0u32;
        for w in 0..words {
            y += (m[w] & rem[w]).count_ones();
        }
        if y == 0 || (y as i64) * 100 <= best {
            continue;
        }
        let on = list.on[i];
        let mut d = 0usize;
        let mut off_needed = false;
        for e in 0..6 {
            let active = (st.act >> e) & 1 != 0;
            if (on >> e) & 1 != 0 {
                let a = st.pos[e] as usize;
                let b = list.pos[i][e] as usize;
                let rd = if a > b { a - b } else { b - a };
                let rd = rd.min(n - rd) + (!active) as usize;
                d = d.max(rd);
            } else if active {
                off_needed = true;
            }
        }
        if off_needed {
            d = d.max(1);
        }
        let rate = y as i64 * 100 / (d as i64 + 1);
        if rate > best {
            best = rate;
        }
    }
    best
}

thread_local! {
    static DIV: usize = env_num("DIV", 0) as usize;
}

/// Maximum number of beam states sharing the same set of created cells (0 = unlimited).
pub fn diversity() -> usize {
    DIV.with(|v| *v)
}

thread_local! {
    static PFAM: bool = env_num("PFAM", 0) != 0;
}

/// Potential counts only alignments of two different line families (no line fills).
pub fn family_potential() -> bool {
    PFAM.with(|v| *v)
}

thread_local! {
    static VWHOLE: bool = env_num("VWHOLE", 0) != 0;
}

/// VWHOLE=1: variants compete on whole programs (all copies) instead of per copy.
pub fn whole_program_variants() -> bool {
    VWHOLE.with(|v| *v)
}
