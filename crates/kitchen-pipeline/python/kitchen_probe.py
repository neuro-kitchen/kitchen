"""Checks a script without data: it imports, and its operation declares what it does.

``python kitchen_probe.py <script> <alias,alias>`` prints one JSON line:
``{"ok": true, "raw": <kitchen_pipeline.describe(fn)>}`` or ``{"ok": false, "error": "...", "line": n}``.
Run with the folder holding ``kitchen_pipeline.py`` on ``sys.path``.
"""

import json
import os
import sys
import traceback

import kitchen_pipeline as kp
import kitchen_runtime as rt


def where(script: str, exc: BaseException):
    if isinstance(exc, SyntaxError) and exc.lineno:
        return exc.lineno
    for frame in reversed(traceback.extract_tb(exc.__traceback__)):
        if os.path.abspath(frame.filename) == os.path.abspath(script):
            return frame.lineno
    return None


def main() -> None:
    script, aliases = sys.argv[1], [a for a in sys.argv[2].split(",") if a]
    sys.path.insert(0, os.path.dirname(os.path.abspath(script)))
    rt.install(aliases)
    try:
        module = rt.load(script)
        fn = kp.find(module)
        if fn is None:
            raise LookupError("no operation: decorate a function with @op, or define run()")
        out = {"ok": True, "raw": kp.describe(fn)}
    except BaseException as e:  # noqa: BLE001 - the user's code may raise anything
        out = {"ok": False, "error": "".join(traceback.format_exception_only(type(e), e)).strip(), "line": where(script, e)}
    sys.stdout.write(json.dumps(out) + "\n")


if __name__ == "__main__":
    main()
