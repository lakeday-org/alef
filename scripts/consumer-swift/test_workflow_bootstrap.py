"""Proposed source-contract controls; no bootstrap, network or hosted build runs here."""

import re
import unittest
from pathlib import Path

from assertions import raises


def require_bootstrap_before_task(workflow):
    steps = re.split(r"(?m)^      - ", workflow)[1:]
    bootstrap = [
        i
        for i, step in enumerate(steps)
        if "run: python3 gate/scripts/consumer-swift/entry.py --bootstrap-task\n" in step
    ]
    tasks = [i for i, step in enumerate(steps) if "run: task --dir gate consumer-swift:verify\n" in step]
    if len(bootstrap) != 1 or len(tasks) != 1 or bootstrap[0] >= tasks[0]:
        raise ValueError("verified Task bootstrap must precede its actual Task invocation")
    if "ref: ${{ github.sha }}\n" not in steps[0] or "path: gate\n" not in steps[0]:
        raise ValueError("workflow harness checkout must use its immutable invocation commit")


class WorkflowBootstrap(unittest.TestCase):
    def workflow(self):
        return (Path(__file__).resolve().parents[2] / ".github/workflows/verify-xberg-swift.yaml").read_text()

    def test_actual_workflow_bootstraps_before_calling_task(self):
        require_bootstrap_before_task(self.workflow())

    def test_missing_and_late_bootstrap_are_rejected_for_order(self):
        source = self.workflow()
        prefix, *steps = re.split(r"(?m)^      - ", source)
        bootstrap = next(step for step in steps if "--bootstrap-task\n" in step)
        without = [step for step in steps if step != bootstrap]
        for mutated in [prefix + "      - ".join(["", *without]), prefix + "      - ".join(["", *without, bootstrap])]:
            with self.subTest(mutated=mutated), raises(ValueError, "bootstrap must precede"):
                require_bootstrap_before_task(mutated)
