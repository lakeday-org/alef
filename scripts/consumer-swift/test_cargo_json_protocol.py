"""Strict Cargo JSON framing, caller verbosity, and existing compatibility."""

import json
import unittest

import contracts
from assertions import raises


class CargoJsonProtocol(unittest.TestCase):
    def test_build_keeps_structured_diagnostics_and_single_verbose_command_trace(self):
        original = ["build", "--release", "-p", "alef"]
        arguments = contracts.cargo_arguments(original)
        assert arguments == [*original, "--locked", "--message-format=json", "-v"], arguments

    def test_caller_very_verbose_requires_review_before_forwarding(self):
        for verbosity in [["-vv"], ["-vvv"], ["-v", "-v"], ["--verbose", "--verbose"], ["-v", "--verbose"]]:
            try:
                contracts.cargo_arguments(["build", *verbosity])
            except ValueError as error:
                message = str(error)
            else:
                raise AssertionError("very verbose Cargo stdout was accepted")
            assert message == "very verbose Cargo output requires JSON protocol review", message

    def test_compatibility_and_malformed_json_remain_fail_closed(self):
        for verbosity in ["-v", "--verbose"]:
            original = ["check", "--locked", "--features", "one,two", verbosity]
            arguments = contracts.cargo_arguments(original)
            assert arguments[: len(original)] == original
            assert arguments.count("--locked") == 1
            assert arguments.count(verbosity) == 1
        metadata = ["metadata", "--format-version", "1"]
        assert contracts.cargo_arguments(metadata) == metadata
        with raises(ValueError, "duplicate lock policy"):
            contracts.cargo_arguments(["build", "--locked", "--locked"])
        with raises(ValueError, "unexpected caller message format"):
            contracts.cargo_arguments(["build", "--message-format=json"])
        malformed = '{"reason": malformed}'
        try:
            contracts.cargo_receipt(['{"reason":"build-finished","success":true}', malformed], "binding", False)
        except json.JSONDecodeError as error:
            document = error.doc
        else:
            raise AssertionError("malformed Cargo JSON was discarded")
        assert document == malformed
