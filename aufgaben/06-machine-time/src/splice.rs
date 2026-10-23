//! Submission-graph splicer: recombine segments of all candidate programs for a pattern.
//!
//! Right after a submission the pattern cells and their halo are empty and no other cell can
//! influence a later submission, so the state at a submission is just the emitter configuration
//! (ring positions + active bits). The lines between two submissions therefore form an edge
//! `cfg_a -> cfg_b` that works from any program reaching `cfg_a`. We collect the cheapest edge per
//! pair over all candidates and run a DP for the cheapest walk with exactly `k` edges from START.

use crate::{geometry, home, simulate, Geometry};
use std::collections::HashMap;

type Node = u64;

fn encode(idx: &[usize; 6], act: &[bool; 6]) -> Node {
    let mut v: u64 = 0;
    for e in 0..6 {
        v = (v << 8) | idx[e] as u64;
    }
    let a = (0..6).fold(0u64, |a, e| a | ((act[e] as u64) << e));
    (v << 8) | a
}

/// Replay one program; returns (from, to, lines) for every submission segment.
fn edges_of(g: &Geometry, r: usize, prog: &[[u8; 6]], k: u32) -> Vec<(Node, Node, Vec<[u8; 6]>)> {
    let n = g.n;
    let mut idx = home(r);
    let mut active = [false; 6];
    let mut matter = vec![false; g.ncell];
    let mut hits = vec![0u8; g.ncell];
    let mut touched: Vec<u16> = Vec::new();
    let mut prev = encode(&idx, &active);
    let mut seg_start = 0;
    let mut out = Vec::new();
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
            if active[e] {
                for &c in &g.ray[e][idx[e]] {
                    if hits[c as usize] == 0 {
                        touched.push(c);
                    }
                    hits[c as usize] += 1;
                }
            }
        }
        for &c in &touched {
            match hits[c as usize] {
                2 => matter[c as usize] = true,
                3.. => matter[c as usize] = false,
                _ => {}
            }
            hits[c as usize] = 0;
        }
        let full = g.pat.iter().all(|&c| matter[c as usize]);
        if full && (0..g.ncell).all(|c| !g.forbidden[c] || !matter[c]) {
            for &c in &g.pat {
                matter[c as usize] = false;
            }
            let node = encode(&idx, &active);
            out.push((prev, node, prog[seg_start..=t].to_vec()));
            prev = node;
            seg_start = t + 1;
            if out.len() as u32 >= k {
                break;
            }
        }
    }
    out
}

/// Cheapest program with exactly `k` submissions spliced from `cands` (each a full program for
/// this pattern). Returns None if nothing shorter than `best_len` exists.
pub fn splice(r: usize, pattern: &[Vec<u8>], k: u32, cands: &[&[[u8; 6]]], best_len: usize) -> Option<Vec<[u8; 6]>> {
    let g = geometry(r, pattern);
    let mut ids: HashMap<Node, usize> = HashMap::new();
    let start = encode(&home(r), &[false; 6]);
    ids.insert(start, 0);
    let mut edges: HashMap<(usize, usize), Vec<[u8; 6]>> = HashMap::new();
    for prog in cands {
        for (a, b, lines) in edges_of(&g, r, prog, k) {
            let next = ids.len();
            let ia = *ids.entry(a).or_insert(next);
            let next = ids.len();
            let ib = *ids.entry(b).or_insert(next);
            let e = edges.entry((ia, ib)).or_insert_with(|| lines.clone());
            if lines.len() < e.len() {
                *e = lines;
            }
        }
    }
    let nn = ids.len();
    let edge_list: Vec<((usize, usize), usize)> = edges.iter().map(|(&ab, l)| (ab, l.len())).collect();
    // dist[j][node] = (cost, prev node) with exactly j submissions
    let mut dist: Vec<Vec<Option<(usize, usize)>>> = vec![vec![None; nn]; k as usize + 1];
    dist[0][0] = Some((0, usize::MAX));
    for j in 1..=k as usize {
        for &((a, b), c) in &edge_list {
            if let Some((da, _)) = dist[j - 1][a] {
                let nd = da + c;
                if dist[j][b].map_or(true, |(db, _)| nd < db) {
                    dist[j][b] = Some((nd, a));
                }
            }
        }
    }
    let (end, (cost, _)) = dist[k as usize]
        .iter()
        .enumerate()
        .filter_map(|(i, d)| d.map(|d| (i, d)))
        .min_by_key(|(_, d)| d.0)?;
    if cost >= best_len {
        return None;
    }
    // reconstruct
    let mut path = Vec::new();
    let mut node = end;
    for j in (1..=k as usize).rev() {
        let (_, prev) = dist[j][node].unwrap();
        path.push((prev, node));
        node = prev;
    }
    path.reverse();
    let mut prog = Vec::with_capacity(cost);
    for ab in path {
        prog.extend_from_slice(&edges[&ab]);
    }
    (prog.len() == cost && simulate(r, pattern, &prog) == k).then_some(prog)
}
