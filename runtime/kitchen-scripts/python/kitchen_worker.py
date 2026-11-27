"""The warm worker: one Python process kept alive between pieces, so imports (NumPy, SciPy,
PyTorch) are paid once. JSON lines on stdin and stdout.

Request: ``{"script": path, "dir": folder}``; the folder holds ``request.json``::

    {"function": "local_contrast_boost",
     "inputs": {"image": {"file": "image.npy", "format": "npy"}},
     "params": {"sigma": 3.0},
     "piece": {"index": 4, "own": {...}, "read": {...}} or null,
     "outputs": ["image"]}

Arrays are mapped from the files without a copy (``/dev/shm``). The function is called as
``fn(**inputs, **params[, tile=piece])``; with one output it returns the value, with several a dict
by output name. Arrays are written ``<name>.out.npy``, tables ``.csv``, anything else ``.json``.
Answer: ``{"ok": true, "outputs": {"image": {"file": "image.out.npy", "format": "npy"}}}`` or
``{"ok": false, "error": "<traceback>"}``. A changed script is reloaded; its ``init()`` runs once
per version.
"""

import csv
import json
import os
import sys
import traceback

import kitchen_pipeline as kp
import kitchen_runtime as rt

try:
    import numpy as np
except ImportError:  # tables and JSON still work
    np = None

loaded = {}  # script -> (sha, module)


def read_input(folder, spec):
    path = os.path.join(folder, spec["file"])
    fmt = spec["format"]
    if fmt == "npy":
        return np.load(path, mmap_mode="r")
    if fmt == "csv":
        for lib in ("polars", "pandas"):
            try:
                mod = __import__(lib)
                return mod.read_csv(path)
            except ImportError:
                continue
        with open(path, newline="") as f:
            return list(csv.DictReader(f))
    with open(path) as f:
        return json.load(f)


def write_output(folder, name, value):
    if np is not None and (isinstance(value, np.ndarray) or hasattr(value, "__array__")) and not hasattr(value, "to_csv") and not hasattr(value, "write_csv"):
        file = name + ".out.npy"
        np.save(os.path.join(folder, file), np.ascontiguousarray(np.asarray(value)))
        return {"file": file, "format": "npy"}
    file = name + ".out.csv"
    path = os.path.join(folder, file)
    if hasattr(value, "write_csv"):  # polars
        value.write_csv(path)
        return {"file": file, "format": "csv"}
    if hasattr(value, "to_csv"):  # pandas
        value.to_csv(path, index=False)
        return {"file": file, "format": "csv"}
    if isinstance(value, list) and value and all(isinstance(r, dict) for r in value):
        with open(path, "w", newline="") as f:
            w = csv.DictWriter(f, fieldnames=list(dict.fromkeys(k for r in value for k in r)))
            w.writeheader()
            w.writerows(value)
        return {"file": file, "format": "csv"}
    file = name + ".out.json"
    with open(os.path.join(folder, file), "w") as f:
        json.dump(value, f, default=float)
    return {"file": file, "format": "json"}


def module_of(script):
    sha = rt.sha256(script)
    entry = loaded.get(script)
    if entry is None or entry[0] != sha:
        sys.path.insert(0, os.path.dirname(os.path.abspath(script)))
        module = rt.load(script, sha)
        if callable(getattr(module, "init", None)):
            module.init()
        loaded[script] = (sha, module)
        return module
    return entry[1]


def call(req):
    folder = req["dir"]
    with open(os.path.join(folder, "request.json")) as f:
        r = json.load(f)
    module = module_of(req["script"])
    fn = getattr(module, r["function"], None) or kp.find(module)
    kwargs = {name: read_input(folder, spec) for name, spec in r["inputs"].items()}
    kwargs.update(r["params"])
    if r.get("piece") is not None and r.get("takes_piece"):
        kwargs["tile" if "tile" in fn.__code__.co_varnames else "piece"] = r["piece"]
    result = fn(**kwargs)
    names = r["outputs"]
    if len(names) == 1 and not (isinstance(result, dict) and set(result) == set(names)):
        result = {names[0]: result}
    if not isinstance(result, dict):
        raise TypeError(f"returned {type(result).__name__}; with several outputs return a dict of {', '.join(names)}")
    missing = [n for n in names if n not in result]
    if missing:
        raise KeyError(f"no result for {', '.join(missing)}")
    return {n: write_output(folder, n, result[n]) for n in names}


def main():
    rt.install([a for a in sys.argv[1].split(",") if a])
    for line in sys.stdin:
        line = line.strip()
        if not line:
            continue
        try:
            out = {"ok": True, "outputs": call(json.loads(line))}
        except BaseException:  # noqa: BLE001
            out = {"ok": False, "error": traceback.format_exc()}
        sys.stdout.write(json.dumps(out) + "\n")
        sys.stdout.flush()


if __name__ == "__main__":
    main()
