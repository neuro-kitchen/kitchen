//! The engine over a lucem-like 2-D domain (tiles with a margin): a Rust operation and a decorated
//! Python script in one graph, checked, confirmed, run tile by tile, equal to the whole image.

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::{Arc, Mutex};

use kitchen_engine::domain::tiles;
use kitchen_scripts::{Approvals, Options, PythonWorker, ScriptOp, Tools, probe};
use kitchen_engine::{Call, Cancel, Context, Domain, Engine, Exchange, ExchangeFile, FnOp, Graph, Node, OpSpec, ParamSpec, ParamValue, Piece, PortSpec, PortType, Result};

/// An image as rows of numbers, over a region of the whole.
#[derive(Clone, Debug, PartialEq)]
struct Block {
    h: usize,
    w: usize,
    data: Vec<f64>,
}

impl Block {
    fn at(&self, y: usize, x: usize) -> f64 {
        self.data[y * self.w + x]
    }
}

struct Slide {
    image: Block,
    tile: usize,
    out: Block,
}

impl Domain for Slide {
    type Data = Block;
    fn name(&self) -> &str {
        "slide"
    }
    fn port_types(&self) -> Vec<PortType> {
        vec![PortType::new("image")]
    }
    fn pieces(&self, _: &Graph, context: Context) -> Result<Vec<Piece>> {
        Ok(tiles([1, self.image.h, self.image.w], [1, self.tile, self.tile], context))
    }
    fn read(&self, _: &str, _: &PortType, piece: &Piece) -> Result<Block> {
        let r = piece.read;
        let mut data = Vec::new();
        for y in r.start[1]..r.end()[1] {
            for x in r.start[2]..r.end()[2] {
                data.push(self.image.at(y, x));
            }
        }
        Ok(Block { h: r.size[1], w: r.size[2], data })
    }
    fn trim(&self, b: Block, _: &PortType, piece: &Piece) -> Result<Block> {
        let [_, ys, xs] = piece.inner();
        let mut data = Vec::new();
        for y in ys.clone() {
            for x in xs.clone() {
                data.push(b.at(y, x));
            }
        }
        Ok(Block { h: ys.len(), w: xs.len(), data })
    }
    fn write(&mut self, _: &str, _: &PortType, piece: &Piece, b: Block) -> Result<()> {
        let o = piece.own;
        for y in 0..b.h {
            for x in 0..b.w {
                self.out.data[(o.start[1] + y) * self.out.w + o.start[2] + x] = b.at(y, x);
            }
        }
        Ok(())
    }
    fn exchange(&self) -> Option<&dyn Exchange<Block>> {
        Some(&JsonExchange)
    }
}

/// Blocks to scripts as JSON rows; results back as `.npy` (float64 or float32).
struct JsonExchange;

impl Exchange<Block> for JsonExchange {
    fn put(&self, b: &Block, _: &PortType, dir: &Path, name: &str) -> Result<ExchangeFile> {
        let rows: Vec<&[f64]> = b.data.chunks(b.w).collect();
        let file = format!("{name}.json");
        std::fs::write(dir.join(&file), serde_json::to_string(&rows).unwrap()).unwrap();
        Ok(ExchangeFile { file, format: "json".into() })
    }
    fn take(&self, f: &ExchangeFile, _: &PortType, dir: &Path, like: Option<&Block>) -> Result<Block> {
        let bytes = std::fs::read(dir.join(&f.file)).unwrap();
        let len = u16::from_le_bytes([bytes[8], bytes[9]]) as usize;
        let header = std::str::from_utf8(&bytes[10..10 + len]).unwrap();
        let body = &bytes[10 + len..];
        let data: Vec<f64> = if header.contains("<f8") { body.chunks_exact(8).map(|c| f64::from_le_bytes(c.try_into().unwrap())).collect() } else { body.chunks_exact(4).map(|c| f32::from_le_bytes(c.try_into().unwrap()) as f64).collect() };
        let like = like.expect("an input");
        Ok(Block { h: like.h, w: like.w, data })
    }
}

/// A 3 × 3 box mean (edges clamped): needs 1 px around.
fn box_mean(b: &Block) -> Block {
    let mut data = Vec::with_capacity(b.data.len());
    for y in 0..b.h {
        for x in 0..b.w {
            let (mut s, mut n) = (0.0, 0.0);
            for dy in -1i64..=1 {
                for dx in -1i64..=1 {
                    let (yy, xx) = (y as i64 + dy, x as i64 + dx);
                    if yy >= 0 && xx >= 0 && (yy as usize) < b.h && (xx as usize) < b.w {
                        s += b.at(yy as usize, xx as usize);
                        n += 1.0;
                    }
                }
            }
            data.push(s / n);
        }
    }
    Block { h: b.h, w: b.w, data }
}

fn slide() -> Slide {
    let (h, w) = (37, 50);
    let data = (0..h * w).map(|i| ((i * 31) % 17) as f64).collect();
    Slide { image: Block { h, w, data }, tile: 16, out: Block { h, w, data: vec![0.0; h * w] } }
}

fn engine(s: &Slide) -> Engine<Slide> {
    let mut e = Engine::new(s);
    let spec = OpSpec::new("box_mean", "Box mean").family("Filter").input(PortSpec::new("image", "image")).output(PortSpec::new("image", "image")).context(Context::around(1, 2));
    e.add(FnOp::new(spec, |mut call: Call<'_, Slide>| Ok(BTreeMap::from([("image".to_string(), box_mean(&call.take("image")?))])))).unwrap();
    let spec = OpSpec::new("scale", "Scale").input(PortSpec::new("image", "image")).output(PortSpec::new("image", "image")).param(ParamSpec::number("k", 1.0));
    e.add(FnOp::new(spec, |mut call: Call<'_, Slide>| {
        let k = call.number("k")?;
        let mut b = call.take("image")?;
        b.data.iter_mut().for_each(|v| *v *= k);
        Ok(BTreeMap::from([("image".to_string(), b)]))
    }))
    .unwrap();
    e
}

fn graph(second: &str) -> Graph {
    let mut g = Graph::new("slide");
    g.sources.insert("dapi".into(), PortType::new("image"));
    g.nodes.insert("mean".into(), Node::new("box_mean"));
    g.nodes.insert("second".into(), Node::new(second).with("k", ParamValue::Number(2.0)));
    g.connect("dapi".parse().unwrap(), "mean.image".parse().unwrap());
    g.connect("mean.image".parse().unwrap(), "second.image".parse().unwrap());
    g.outputs.insert("result".into(), "second.image".parse().unwrap());
    g
}

#[test]
fn tiles_with_a_margin_give_the_whole_image_result() {
    let mut s = slide();
    let e = engine(&s);
    let report = e.run(&graph("scale"), &mut s, &Cancel::new(), &mut |_| {}).unwrap();
    assert_eq!(report.pieces, 12, "3 × 4 tiles");
    let mut whole = box_mean(&s.image);
    whole.data.iter_mut().for_each(|v| *v *= 2.0);
    assert_eq!(s.out, whole, "no seams");
}

/// A decorated Python script, checked and confirmed, runs as a node (skipped without NumPy).
#[test]
fn a_confirmed_python_script_runs_as_a_node() {
    let tools = Tools::detect();
    let numpy = std::process::Command::new(&tools.python).args(["-c", "import numpy"]).output().is_ok_and(|o| o.status.success());
    if !numpy {
        eprintln!("skipped: no Python with NumPy");
        return;
    }
    let dir = std::env::temp_dir().join(format!("kitchen-engine-script-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("scale.py");
    std::fs::write(&path, "import numpy as np\nimport slide\n\n@slide.op(family=\"Math\")\n@slide.param(\"k\", 1.0, min=0.0, max=10.0)\ndef scale(image: slide.Image, k: float = 1.0) -> slide.Image:\n    \"\"\"Multiplies by k.\"\"\"\n    return np.asarray(image, dtype=np.float64) * k\n").unwrap();
    let opts = Options::new("slide", &["image"], "image");
    let script = kitchen_scripts::Script::read(&path, &opts).unwrap();
    assert!(script.problems.is_empty(), "{:?}", script.problems);

    // Not runnable until checked (decision 0016: checked is enough until the contract's v0.2)
    let approvals = Approvals::new();
    assert!(!approvals.can_run(&approvals.status(&script, None)));
    let checked = probe(&script, &tools, &opts);
    assert!(checked.passed(), "{checked:?}");
    assert!(approvals.can_run(&approvals.status(&script, Some(&checked))));
    // With a confirming policy (v0.2) it waits for a person
    let mut confirming = Approvals::new();
    confirming.policy = kitchen_scripts::ConfirmPolicy::Content;
    assert!(!confirming.can_run(&confirming.status(&script, Some(&checked))));
    confirming.confirm(&script, Some(&checked), "test").unwrap();
    assert!(confirming.can_run(&confirming.status(&script, Some(&checked))));

    let mut s = slide();
    let mut e = engine(&s);
    let worker = Arc::new(Mutex::new(PythonWorker::new(&tools, &opts)));
    e.add(ScriptOp::new(&script, Some(worker), None).unwrap()).unwrap();
    assert_eq!(e.catalog().get("script:scale").unwrap().family, "Math");
    let report = e.run(&graph("script:scale"), &mut s, &Cancel::new(), &mut |_| {}).unwrap();
    assert_eq!(report.scripts["second"], script.hash, "the run records the script's version");
    let mut whole = box_mean(&s.image);
    whole.data.iter_mut().for_each(|v| *v *= 2.0);
    assert_eq!(s.out, whole);
}
