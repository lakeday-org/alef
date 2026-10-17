"""Dependency-free exception assertions for the hosted bootstrap controls."""

import re
from contextlib import contextmanager


@contextmanager
def raises(expected, match=None):
    try:
        yield
    except expected as error:
        if match is not None and re.search(match, str(error)) is None:
            raise AssertionError from error
    else:
        raise AssertionError("expected exception was not raised")
