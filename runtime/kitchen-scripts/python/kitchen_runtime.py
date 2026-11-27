"""What the probe and the worker share: the app's names for the decorators, and loading a script.

Each alias (``kitchen``, ``lucem``, ``dspk``) becomes a module whose decorators are
:mod:`kitchen_pipeline`'s. When the app's real package is installed, everything else
(``lucem.io``, ``dspk.ops``) still comes from it; when it is not, the script runs anyway.
"""

from __future__ import annotations

import hashlib
import importlib
import importlib.util
import sys
import types

import kitchen_pipeline as kp


def install(aliases: list[str]) -> None:
    for alias in aliases:
        try:
            real = importlib.import_module(alias)
        except Exception:
            real = None
        shim = types.ModuleType(alias)
        for name in kp.__all__:
            setattr(shim, name, getattr(kp, name))
        shim.transform = kp.op
        if real is not None:
            shim.__getattr__ = lambda name, real=real: getattr(real, name)
            if hasattr(real, "__path__"):
                shim.__path__ = real.__path__
        sys.modules[alias] = shim


def sha256(path: str) -> str:
    with open(path, "rb") as f:
        return hashlib.sha256(f.read()).hexdigest()


def load(path: str, digest: str | None = None):
    """Imports the script at ``path`` as a fresh module (named by its content, so an edit reloads)."""
    digest = digest or sha256(path)
    spec = importlib.util.spec_from_file_location("kitchen_script_" + digest[:16], path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module
