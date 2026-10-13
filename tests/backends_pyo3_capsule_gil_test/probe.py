"""Exercise Python thread progress through generated capsule wrappers."""

import sys
import threading
import types
from collections.abc import Callable
from typing import Any


class Handle:
    """Hold the generated capsule's owning Python reference."""

    def __init__(self, capsule: object) -> None:
        self.capsule = capsule


class Parser(Handle):
    """Model a Python-side factory receiving an existing handle."""


def _probe_call(call: Callable[[str], Handle], core_active: Callable[[], bool], expected_release: bool) -> None:
    ready = threading.Event()
    failures: list[BaseException] = []

    def _invoke() -> None:
        ready.set()
        try:
            if not isinstance(call("fixture"), Handle):
                raise AssertionError("native call did not construct the Python handle")
        except BaseException as error:
            failures.append(error)

    worker = threading.Thread(target=_invoke)
    worker.start()
    if not ready.wait(5):
        raise AssertionError("native call did not start")
    observed_core = False
    while worker.is_alive():
        observed_core |= core_active()
    worker.join()
    if failures:
        raise AssertionError("native call failed") from failures[0]
    if observed_core != expected_release:
        raise AssertionError((call, observed_core, expected_release))


def _probe_error(call: Callable[[str], Handle]) -> None:
    try:
        call("bad")
    except RuntimeError as error:
        if "bad handle request" not in str(error):
            raise AssertionError("native error lost its context") from error
    else:
        raise AssertionError("core error did not reach Python")


def _run(native: Any, expected_release: bool) -> None:
    capsule_module = types.ModuleType("capsule_probe")
    capsule_module.Handle = Handle
    capsule_module.Parser = Parser
    sys.modules["capsule_probe"] = capsule_module
    for call in [
        native.make_handle,
        native.get_handle,
        native.registry.get_handle,
        native.get_parser,
        lambda _name: native.empty_handle(),
    ]:
        _probe_call(call, native.core_active, expected_release)
    for call in [native.get_handle, native.registry.get_handle]:
        _probe_error(call)


_run(globals()["native"], bool(globals()["expected_release"]))
