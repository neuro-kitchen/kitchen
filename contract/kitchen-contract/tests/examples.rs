//! The examples of the standard, as data (`standard/examples/fixtures/`), read and checked: what
//! each walkthrough says on paper, checked on every pull request.

use std::collections::BTreeMap;
use std::path::PathBuf;

use kitchen_contract::{Catalog, ContentId, Margin, OpSpec, ParamValue, Pipeline, RunKey, Type, check_params};

fn fixture(name: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../standard/examples/fixtures").join(name);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

fn catalog(file: &str, types: &[&str]) -> Catalog {
    let ops: Vec<OpSpec> = serde_json::from_str(&fixture(file)).unwrap();
    let mut c = Catalog::new(types.iter().map(Type::new));
    for op in ops {
        let id = op.id.clone();
        c.add(op).unwrap_or_else(|e| panic!("{id}: {e}"));
    }
    c
}

fn dspk() -> Catalog {
    catalog("dspk-ops.json", &["signal", "events", "table", "figure", "file", "dspk:unit", "dspk:sorting", "dspk:whitening"])
}

fn lucem() -> Catalog {
    catalog("lucem-ops.json", &["image", "mask", "labels", "table", "region"])
}

/// 01: Claude's script op is described like any other, versioned by its hash, and listed where
/// Analysis's offers can fill it, and nowhere they cannot.
#[test]
fn example_01_a_script_op_appears_where_it_fits() {
    let c = dspk();
    let plot = c.get("script:firing_rate_by_condition").unwrap();
    assert!(plot.version_key().starts_with("sha256:"), "a script is versioned by its hash");
    let analysis = [Type::new("dspk:unit"), Type::new("events")];
    let fits: Vec<&str> = c.fillable_by(&analysis).iter().map(|o| o.id.as_str()).collect();
    assert_eq!(fits, ["script:firing_rate_by_condition"], "kilosort needs a recording");
    // Running it: parameters checked and filled, a run key from the hash, the params and the inputs
    let given: BTreeMap<String, ParamValue> = [("bin_ms".to_string(), ParamValue::Integer(250))].into();
    let params = check_params(&plot.params, &given).unwrap();
    assert_eq!(params["condition"], ParamValue::Text("condition".into()));
    let unit = ContentId::of_bytes(b"unit 17");
    let trials = ContentId::of_bytes(b"trials of session 12");
    let inputs: BTreeMap<String, ContentId> = [("unit".to_string(), unit), ("trials".to_string(), trials)].into();
    let key = RunKey::of(&plot.id, &plot.version_key(), &params, &inputs);
    assert_eq!(key, RunKey::of(&plot.id, &plot.version_key(), &params, &inputs), "the same run is recognized");
    // Effects: the upload is marked, so it is never reused or previewed
    assert!(c.get("dspk:dandi_upload").unwrap().effects);
}

/// 03: the cell detection pipeline checks; its margin is what the walk reads around each tile;
/// watershed and measure say how their pieces are joined; saved as a recipe it has a hash.
#[test]
fn example_03_a_pipeline_becomes_a_recipe() {
    let c = lucem();
    let p = Pipeline::from_json(&fixture("03-cell-detect.pipeline.json")).unwrap();
    let plan = p.check(&c).unwrap_or_else(|e| panic!("{e:?}"));
    assert_eq!(plan.order, ["denoise", "threshold", "watershed", "measure"]);
    assert_eq!(plan.margin, Margin::around(18, 2), "denoise 6 then watershed 12");
    assert_eq!(c.get("lucem:watershed").unwrap().finish.as_deref(), Some("stitch_labels"));
    let recipe = ContentId::of_json(&serde_json::from_str(&fixture("03-cell-detect.pipeline.json")).unwrap());
    let reread = ContentId::of_json(&serde_json::to_value(&p).unwrap());
    assert_eq!(recipe.as_str().len(), 71);
    assert_eq!(recipe, reread, "reading and writing a pipeline keeps its hash: field order and spacing do not count");
    // A model that is not installed says so
    let sam = c.get("lucem:sam").unwrap();
    assert!(!sam.available && sam.unavailable.as_deref() == Some("weights not downloaded"));
}

/// 04 A: windows of a recording; the fitted whitening is a separate op whose result the apply
/// op takes, and the margins of the filters add up along the pipeline.
#[test]
fn example_04a_windows_with_a_fitted_step() {
    let c = dspk();
    let p = Pipeline::from_json(&fixture("04a-windows.pipeline.json")).unwrap();
    let plan = p.check(&c).unwrap_or_else(|e| panic!("{e:?}"));
    assert_eq!(plan.order.first().map(String::as_str), Some("hp"));
    assert!(plan.order.iter().position(|n| n == "fit") < plan.order.iter().position(|n| n == "white"));
    assert_eq!(plan.margin, Margin::time(3040, 3040), "high-pass 3000 then detection 40");
    assert_eq!(plan.params["hp"]["cutoff_hz"], ParamValue::Number(300.0));
}

/// 04 E: an op of another app is refused in an app's pipeline, saying where it runs.
#[test]
fn example_04e_another_apps_op_is_a_chain() {
    let p = Pipeline::from_json(&fixture("04e-across-apps.pipeline.json")).unwrap();
    let problems = p.check(&dspk()).unwrap_err();
    let text: Vec<String> = problems.iter().map(|p| p.to_string()).collect();
    assert!(text.iter().any(|t| t.starts_with("convert: no operation \"convert:nwb\"") && t.contains("chain")), "{text:?}");
}
