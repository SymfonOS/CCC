use std::{fs, path::Path};

fn board(r: usize) -> String {
    let mut out = String::new();
    for k in 0..=2 * r {
        let m = k.min(2 * r - k);
        let pad = "-".repeat(r - m);
        let edge = k == 0 || k == 2 * r;
        let cells: Vec<&str> = (0..r + 1 + m)
            .map(|i| if edge || i == 0 || i == r + m { "*" } else { "O" })
            .collect();
        out += &format!("{pad}{}{pad}\n", cells.join("-"));
    }
    out
}

fn solve(input: &str) -> String {
    let mut nums = input.split_whitespace().map(|t| t.parse::<usize>().unwrap());
    let n = nums.next().unwrap();
    nums.take(n).map(|r| board(r) + "\n").collect()
}

fn main() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("daten");
    let mut files: Vec<_> = fs::read_dir(&dir).unwrap().flatten().map(|e| e.path()).collect();
    files.sort();
    for p in files {
        let name = p.file_name().unwrap().to_string_lossy().into_owned();
        if let Some(rest) = name.strip_prefix("in_") {
            let res = solve(&fs::read_to_string(&p).unwrap());
            fs::write(dir.join(format!("out_{rest}")), res).unwrap();
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
        let inp = fs::read_to_string(d.join("in_level-1_0-example.txt")).unwrap();
        let exp = fs::read_to_string(d.join("erwartet_level-1_0-example.txt")).unwrap();
        assert_eq!(solve(&inp), exp);
    }
}
