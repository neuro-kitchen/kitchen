"""Make a Python function a pipeline operation, in any neuro-kitchen app.

Each app ships this module under its own name (``lucem.op`` is ``kitchen_pipeline.op``), and the
app's script worker provides it to every script, so a script runs without it installed::

    import numpy as np
    import lucem                       # or: import dspk / import kitchen_pipeline as kitchen

    @lucem.op(family="Filter", halo=12)
    @lucem.param("sigma", 3.0, min=0.2, max=20.0, unit="px")
    def local_contrast_boost(image: lucem.Image, sigma: float = 3.0, strength: float = 1.8) -> lucem.Image:
        \"\"\"Enhance fine structures by subtracting a blurred local background.\"\"\"
        ...

What the app reads from it (and from the file's text, before anything runs):

- **Inputs**: arguments without a default that are not plain values (``image: Image``,
  ``x: np.ndarray``); the type comes from the annotation (``Image``, ``Mask``, ``Signal``…), else
  the argument's name, else the app's main type. ``mask: Mask | None = None`` is optional.
  ``@input(name, type)`` declares one the signature cannot say.
- **Parameters**: arguments with a default (``sigma: float = 3.0``); ``Literal["a", "b"]`` is a
  choice. ``@param(name, default, min=, max=, unit=, choices=, hint=)`` adds a range or a unit.
- **Outputs**: the return annotation (``-> Mask``), or ``@output(name, type)`` for several
  (the function then returns a dict).
- **Context**: ``halo=`` pixels around a tile (every walked axis), ``before=`` / ``after=`` samples
  around a window of time.
- **Its place**: ``name=``, ``family=``, ``description=`` (else the docstring's first line).
- An argument named ``tile`` or ``piece`` receives where the piece sits (it is not an input).

The decorators do not change the function: it is called as written, in Python and in the app.
Before a script runs in a pipeline the app checks it (it imports, and declares what its text
says) and a person confirms it; any change to the file asks again.
"""

from __future__ import annotations

import inspect

__all__ = ["op", "param", "input", "output", "describe", "port", "Image", "Mask", "Labels", "Table", "Signal", "Events"]


def _meta(fn) -> dict:
    meta = fn.__dict__.get("__kitchen__")
    if meta is None:
        meta = {"op": None, "params": [], "inputs": [], "outputs": []}
        fn.__kitchen__ = meta
    return meta


def op(fn=None, *, name=None, family=None, description=None, halo=0, before=0, after=0, output=None, **extra):
    """Marks ``fn`` as an operation (``@op`` or ``@op(...)``). Unknown keywords are kept for the
    app (lucem's ``volume=``, ``block=``)."""

    def mark(f):
        args = {"name": name, "family": family, "description": description, "halo": halo, "before": before, "after": after, "output": output}
        args.update(extra)
        _meta(f)["op"] = {k: v for k, v in args.items() if v is not None}
        return f

    return mark(fn) if callable(fn) else mark


def param(name, default=None, *, min=None, max=None, unit=None, choices=None, hint=None):  # noqa: A002
    """Declares or refines the parameter ``name``: its default, range, unit, choices, a line for people."""

    def mark(f):
        d = {"name": name, "default": default, "min": min, "max": max, "unit": unit, "choices": list(choices) if choices else None, "hint": hint}
        _meta(f)["params"].insert(0, {k: v for k, v in d.items() if v is not None})
        return f

    return mark


def input(name, type, *, optional=False):  # noqa: A001, A002
    """Declares the input ``name`` of type ``type`` (``"image"``, ``"signal"``)."""

    def mark(f):
        _meta(f)["inputs"].insert(0, {"name": name, "type": str(type), "optional": bool(optional)})
        return f

    return mark


def output(name, type):  # noqa: A002
    """Declares the output ``name`` of type ``type``; with several, the function returns a dict."""

    def mark(f):
        _meta(f)["outputs"].insert(0, {"name": name, "type": str(type)})
        return f

    return mark


def port(type_name: str):
    """A type to annotate with: ``def f(x: port("signal"))``. ``Image``, ``Signal``… are made this way."""
    return type(type_name.capitalize(), (), {"__kitchen_port__": type_name.lower()})


Image = port("image")
Mask = port("mask")
Labels = port("labels")
Table = port("table")
Signal = port("signal")
Events = port("events")


def _annotation(a) -> str | None:
    if a is inspect.Parameter.empty or a is inspect.Signature.empty:
        return None
    if isinstance(a, str):
        return a
    kitchen_port = getattr(a, "__kitchen_port__", None)
    if kitchen_port:
        return "port:" + kitchen_port
    return getattr(a, "__name__", None) if getattr(a, "__module__", "") == "builtins" else repr(a)


def describe(fn) -> dict:
    """What the app reads of ``fn``, from the function itself (the probe's answer). Values are
    Python literals as text (``repr``), as the app reads them from the file."""
    meta = getattr(fn, "__kitchen__", None) or {"op": None, "params": [], "inputs": [], "outputs": []}
    sig = inspect.signature(fn)
    args = []
    for p in sig.parameters.values():
        if p.kind in (p.VAR_POSITIONAL, p.VAR_KEYWORD):
            continue
        args.append({"name": p.name, "annotation": _annotation(p.annotation), "default": None if p.default is p.empty else repr(p.default)})

    def lit(d: dict) -> dict:
        return {k: repr(v) for k, v in d.items()}

    doc = inspect.getdoc(fn) or ""
    return {
        "function": fn.__name__,
        "doc": doc.strip().splitlines()[0] if doc.strip() else None,
        "op": lit(meta["op"]) if meta.get("op") is not None else None,
        "args": args,
        "returns": _annotation(sig.return_annotation),
        "params": [lit(d) for d in meta["params"]],
        "inputs": [lit(d) for d in meta["inputs"]],
        "outputs": [lit(d) for d in meta["outputs"]],
    }


def find(module):
    """The module's operation: the function marked with ``@op`` (the first), else ``run``."""
    for v in vars(module).values():
        if callable(v) and (getattr(v, "__kitchen__", None) or {}).get("op") is not None:
            return v
        # lucem's own wheel wraps the function (``Transform``) and marks it
        if callable(v) and getattr(v, "_lucem_op", False):
            return getattr(v, "fn", v)
    return getattr(module, "run", None)
