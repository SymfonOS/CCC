use std::{fs, path::Path};

fn shoot(grid: &mut [Vec<u8>]) {
    let (mut r, mut c, d) = 'find: {
        for (r, row) in grid.iter().enumerate() {
            for (c, &ch) in row.iter().enumerate() {
                if (b'1'..=b'6').contains(&ch) {
                    break 'find (r as i64, c as i64, ch - b'0');
                }
            }
        }
        return;
    };
    let (dr, dc) = match d {
        1 => (0, 2),
        2 => (1, 1),
        3 => (1, -1),
        4 => (0, -2),
        5 => (-1, -1),
        _ => (-1, 1),
    };
    loop {
        r += dr;
        c += dc;
        if r < 0 || c < 0 || r as usize >= grid.len() || c as usize >= grid[r as usize].len() {
            break;
        }
        let cell = &mut grid[r as usize][c as usize];
        if *cell != b'O' {
            break;
        }
        *cell = b'M';
    }
}

fn solve(input: &str) -> String {
    let mut lines = input.lines().map(str::trim_end).filter(|l| !l.is_empty());
    let n: usize = lines.next().unwrap().parse().unwrap();
    let mut out = String::new();
    for _ in 0..n {
        let rows: usize = lines.next().unwrap().split_whitespace().nth(1).unwrap().parse().unwrap();
        let mut grid: Vec<Vec<u8>> = (0..rows).map(|_| lines.next().unwrap().bytes().collect()).collect();
        shoot(&mut grid);
        for row in grid {
            out += std::str::from_utf8(&row).unwrap();
            out.push('\n');
        }
        out.push('\n');
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
        let inp = fs::read_to_string(d.join("in_level-2_0-example.txt")).unwrap();
        let exp = fs::read_to_string(d.join("erwartet_level-2_0-example.txt")).unwrap();
        assert_eq!(solve(&inp), exp);
    }
}
