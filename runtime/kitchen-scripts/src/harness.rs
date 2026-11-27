// The program compiled around a Rust script (kitchen-pipeline). The script is included as the
// module `user` and declares, after its `// kitchen:` header lines:
//
//     pub fn run(inputs: &Inputs, params: &Params) -> Result<Array, String>
//
// `inputs.get("image")?` is an Array (float32, any shape: `[h, w]`, `[channels, samples]`);
// `params.number("sigma")?`, `params.text("mode")?`. The result is the script's first output.
// The program is run with the call's folder: it reads `meta.txt` and the `.npy` inputs there and
// writes the result. Standard library only.

#![allow(dead_code)]

use std::collections::HashMap;
use std::io::Write as _;

/// An n-dimensional array of float32, in row-major order.
#[derive(Clone, Debug, PartialEq)]
pub struct Array {
    pub shape: Vec<usize>,
    pub data: Vec<f32>,
}

impl Array {
    /// `data` with `shape` (their sizes must agree).
    pub fn new(shape: Vec<usize>, data: Vec<f32>) -> Result<Array, String> {
        if shape.iter().product::<usize>() != data.len() {
            return Err(format!("shape {shape:?} holds {} values, not {}", shape.iter().product::<usize>(), data.len()));
        }
        Ok(Array { shape, data })
    }

    /// Zeros of `shape`.
    pub fn zeros(shape: Vec<usize>) -> Array {
        let n = shape.iter().product();
        Array { shape, data: vec![0.0; n] }
    }

    /// `data` with this array's shape.
    pub fn like(&self, data: Vec<f32>) -> Result<Array, String> {
        Array::new(self.shape.clone(), data)
    }

    /// The number of values.
    pub fn len(&self) -> usize {
        self.data.len()
    }

    /// Whether it holds none.
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }
}

/// The script's inputs, by port name.
pub struct Inputs(HashMap<String, Array>);

impl Inputs {
    /// The input `name`.
    pub fn get(&self, name: &str) -> Result<&Array, String> {
        self.0.get(name).ok_or_else(|| format!("no input {name}"))
    }

    /// The input `name`, if connected (optional inputs).
    pub fn maybe(&self, name: &str) -> Option<&Array> {
        self.0.get(name)
    }
}

/// The script's parameters.
pub struct Params {
    numbers: HashMap<String, f64>,
    texts: HashMap<String, String>,
}

impl Params {
    /// The number (or whole number, or switch as 0/1) `name`.
    pub fn number(&self, name: &str) -> Result<f64, String> {
        self.numbers.get(name).copied().ok_or_else(|| format!("no number {name}"))
    }

    /// The text (or choice) `name`.
    pub fn text(&self, name: &str) -> Result<&str, String> {
        self.texts.get(name).map(String::as_str).ok_or_else(|| format!("no text {name}"))
    }
}

fn read_npy(path: &std::path::Path) -> Result<Array, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    if bytes.len() < 10 || &bytes[..6] != b"\x93NUMPY" {
        return Err(format!("{}: not an .npy file", path.display()));
    }
    let (len, start) = if bytes[6] == 1 { (u16::from_le_bytes([bytes[8], bytes[9]]) as usize, 10) } else { (u32::from_le_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]) as usize, 12) };
    let header = std::str::from_utf8(&bytes[start..start + len]).map_err(|e| e.to_string())?;
    let field = |key: &str| -> Option<&str> {
        let at = header.find(&format!("'{key}'"))? + key.len() + 2;
        let rest = header[at..].trim_start().strip_prefix(':')?.trim_start();
        Some(rest)
    };
    let descr = field("descr").and_then(|r| r.strip_prefix('\'')).and_then(|r| r.split('\'').next()).ok_or("no descr")?;
    if field("fortran_order").is_some_and(|r| r.starts_with("True")) {
        return Err("Fortran-ordered arrays are not read: save with np.ascontiguousarray".into());
    }
    let shape_text = field("shape").and_then(|r| r.strip_prefix('(')).and_then(|r| r.split(')').next()).ok_or("no shape")?;
    let shape: Vec<usize> = shape_text.split(',').map(str::trim).filter(|s| !s.is_empty()).map(|s| s.parse::<usize>().map_err(|e| e.to_string())).collect::<Result<_, _>>()?;
    let body = &bytes[start + len..];
    let data: Vec<f32> = match descr {
        "<f4" => body.chunks_exact(4).map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]])).collect(),
        "<f8" => body.chunks_exact(8).map(|c| f64::from_le_bytes(c.try_into().unwrap()) as f32).collect(),
        "<u4" => body.chunks_exact(4).map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]) as f32).collect(),
        "<i4" => body.chunks_exact(4).map(|c| i32::from_le_bytes([c[0], c[1], c[2], c[3]]) as f32).collect(),
        "<u2" => body.chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]]) as f32).collect(),
        "<i2" => body.chunks_exact(2).map(|c| i16::from_le_bytes([c[0], c[1]]) as f32).collect(),
        "|u1" | "|b1" => body.iter().map(|b| *b as f32).collect(),
        other => return Err(format!("{}: arrays of {other} are not read", path.display())),
    };
    Array::new(shape, data)
}

fn write_npy(path: &std::path::Path, a: &Array) -> Result<(), String> {
    let shape = match a.shape.len() {
        1 => format!("({},)", a.shape[0]),
        _ => format!("({})", a.shape.iter().map(|n| n.to_string()).collect::<Vec<_>>().join(", ")),
    };
    let mut header = format!("{{'descr': '<f4', 'fortran_order': False, 'shape': {shape}, }}");
    while (10 + header.len() + 1) % 64 != 0 {
        header.push(' ');
    }
    header.push('\n');
    let mut f = std::fs::File::create(path).map_err(|e| e.to_string())?;
    f.write_all(b"\x93NUMPY\x01\x00").map_err(|e| e.to_string())?;
    f.write_all(&(header.len() as u16).to_le_bytes()).map_err(|e| e.to_string())?;
    f.write_all(header.as_bytes()).map_err(|e| e.to_string())?;
    let mut body = Vec::with_capacity(a.data.len() * 4);
    for v in &a.data {
        body.extend_from_slice(&v.to_le_bytes());
    }
    f.write_all(&body).map_err(|e| e.to_string())
}

fn main() {
    let dir = std::path::PathBuf::from(std::env::args().nth(1).expect("the call's folder"));
    let result = (|| -> Result<(), String> {
        let meta = std::fs::read_to_string(dir.join("meta.txt")).map_err(|e| e.to_string())?;
        let (mut inputs, mut numbers, mut texts, mut output) = (HashMap::new(), HashMap::new(), HashMap::new(), None);
        for line in meta.lines() {
            let mut parts = line.splitn(3, ' ');
            match (parts.next(), parts.next(), parts.next()) {
                (Some("input"), Some(name), Some(file)) => {
                    inputs.insert(name.to_string(), read_npy(&dir.join(file))?);
                }
                (Some("param"), Some(name), Some(v)) => {
                    numbers.insert(name.to_string(), v.parse::<f64>().map_err(|e| format!("{name}: {e}"))?);
                }
                (Some("text"), Some(name), Some(v)) => {
                    texts.insert(name.to_string(), v.to_string());
                }
                (Some("output"), Some(_), Some(file)) => output = Some(file.to_string()),
                _ => {}
            }
        }
        let out = user::run(&Inputs(inputs), &Params { numbers, texts })?;
        write_npy(&dir.join(output.ok_or("no output in meta.txt")?), &out)
    })();
    if let Err(e) = result {
        eprintln!("{e}");
        std::process::exit(1);
    }
}
