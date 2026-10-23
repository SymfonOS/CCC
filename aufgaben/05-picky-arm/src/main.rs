use std::{fs, path::Path};

const DIRS: [(i64, i64); 6] = [(0, 2), (1, 1), (1, -1), (0, -2), (-1, -1), (-1, 1)];
const NEIGHBORS: [(i64, i64); 6] = [(0, 2), (0, -2), (1, 1), (1, -1), (-1, 1), (-1, -1)];

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

/// Number of submitted patterns after running the program.
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
    let mut idx = [5 * r, 0, r, 2 * r, 3 * r, 4 * r];
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

fn solve(input: &str) -> String {
    let mut t = input.split_whitespace();
    let n: usize = t.next().unwrap().parse().unwrap();
    let mut out = String::new();
    for _ in 0..n {
        let r: usize = t.next().unwrap().parse().unwrap();
        let rows: usize = t.next().unwrap().parse().unwrap();
        let _cols = t.next().unwrap();
        let pattern: Vec<Vec<u8>> = (0..rows).map(|_| t.next().unwrap().bytes().collect()).collect();
        let l: usize = t.next().unwrap().parse().unwrap();
        let prog: Vec<[u8; 6]> = (0..l)
            .map(|_| {
                let mut line = [0u8; 6];
                for c in line.iter_mut() {
                    *c = t.next().unwrap().as_bytes()[0];
                }
                line
            })
            .collect();
        out += &format!("{}\n", simulate(r, &pattern, &prog));
    }
    out
}

fn main() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("daten");
    let mut files: Vec<_> = fs::read_dir(&dir).unwrap().flatten().map(|e| e.path()).collect();
    files.sort();
    for p in files {
        let name = p.file_name().unwrap().to_string_lossy().into_owned();
        if let Some(rest) = name.strip_prefix("in_") {
            fs::write(dir.join(format!("out_{rest}")), solve(&fs::read_to_string(&p).unwrap())).unwrap();
            println!("{name}: ok");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn beispiel() {
        let d = Path::new(env!("CARGO_MANIFEST_DIR")).join("daten");
        let inp = fs::read_to_string(d.join("in_level-5_0-example.txt")).unwrap();
        let exp = fs::read_to_string(d.join("erwartet_level-5_0-example.txt")).unwrap();
        assert_eq!(solve(&inp), exp);
    }
}
