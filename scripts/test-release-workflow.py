"""Exercise routing and input guards in the module release workflow."""

import os
from pathlib import Path
import re
import subprocess
import tempfile
import tomllib
import unittest


ROOT = Path(__file__).resolve().parent.parent
WORKFLOW = ROOT / ".github/workflows/release.yml"
CI_WORKFLOW = ROOT / ".github/workflows/ci.yml"
ORDINARY_RELEASE_SHA = "4adc1080fb495098e7af1aade82ed7e6e3c69bb7"
TINYBUS_SDK_SHA = "34d3bd6dda099e54cee11a2f96f28a21da9e0759"


def has_expected_sdk_gitlink(entry, expected_sha):
    """Match the SDK pin from the parent Git tree, even without checkout."""
    fields = entry.split()
    return (
        len(fields) == 4
        and fields[0] == "160000"
        and fields[1] == "commit"
        and fields[2] == expected_sha
        and fields[3] == "vendor/tinybus"
    )


def resolve_script(workflow):
    match = re.search(
        r"(?ms)^      - id: tag\n.*?^        run: \|\n((?:^          [^\n]*\n|^\n)+)",
        workflow,
    )
    if not match:
        raise AssertionError("release resolve script was not found")
    return "\n".join(
        line[10:] if line.startswith("          ") else ""
        for line in match[1].splitlines()
    )


class ReleaseWorkflowTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.workflow = WORKFLOW.read_text()
        cls.ci_workflow = CI_WORKFLOW.read_text()
        cls.script = resolve_script(cls.workflow)

    def run_resolve(self, ref="refs/heads/main", **extra):
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / "output"
            env = dict(
                os.environ,
                TAG="v0.5.2",
                GITHUB_REF=ref,
                GITHUB_OUTPUT=str(output),
                **extra,
            )
            result = subprocess.run(
                ["bash", "-c", self.script],
                env=env,
                text=True,
                capture_output=True,
            )
            return result, output.read_text() if output.exists() else ""

    def test_ordinary_tag_release_route_and_module_ci_pin_are_preserved(self):
        self.assertIn("push:\n    tags: ['v*']", self.workflow)
        self.assertIn("github.event_name == 'push' && github.ref_name || inputs.tag", self.workflow)
        result, output = self.run_resolve()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(output, "tag=v0.5.2\nversion=0.5.2\n")
        self.assertIn(
            f"uses: tinyhumansai/.github/.github/workflows/module-release.yml@{ORDINARY_RELEASE_SHA}",
            self.workflow,
        )
        self.assertIn(
            f"uses: tinyhumansai/.github/.github/workflows/module-ci.yml@{ORDINARY_RELEASE_SHA}",
            self.ci_workflow,
        )
        self.assertIn("coverage-script: scripts/check-coverage.sh", self.ci_workflow)
        self.assertIn("module-e2e-script: scripts/test-e2e.sh", self.ci_workflow)
        self.assertIn("feature-test-script: scripts/test-features.sh", self.ci_workflow)

    def test_release_workflow_test_job_checks_out_pinned_tinybus_submodule(self):
        job = self.ci_workflow.split("  release-workflow-tests:\n", 1)[1].split(
            "  module-ci:\n", 1
        )[0]
        checkout = job.split("      - uses: actions/checkout@", 1)[1].split(
            "      - name:", 1
        )[0]
        self.assertIn("persist-credentials: false", checkout)
        self.assertIn("submodules: recursive", checkout)

    def test_published_candidate_and_sdk_constraints_match_released_compatibility(self):
        manifest = tomllib.loads((ROOT / "Cargo.toml").read_text())
        self.assertRegex(manifest["workspace"]["package"]["version"], r"^\d+\.\d+\.\d+$")
        self.assertEqual(
            manifest["workspace"]["dependencies"]["tinybus"]["version"],
            ">=0.3.6, <0.4",
        )
        self.assertEqual(
            manifest["workspace"]["dependencies"]["tinybus-module"]["version"],
            ">=0.3.6, <0.4",
        )
        sdk_gitlink = subprocess.run(
            ["git", "-C", str(ROOT), "ls-tree", "HEAD", "vendor/tinybus"],
            check=True,
            text=True,
            capture_output=True,
        ).stdout
        self.assertTrue(has_expected_sdk_gitlink(sdk_gitlink, TINYBUS_SDK_SHA))

    def test_sdk_gitlink_guard_accepts_exact_and_rejects_stale_or_missing_pins(self):
        exact = f"160000 commit {TINYBUS_SDK_SHA}\tvendor/tinybus"
        stale = "160000 commit cba2de0e573f99fe4c7e45ba14e2d6b5a2a7757f\tvendor/tinybus"

        self.assertTrue(has_expected_sdk_gitlink(exact, TINYBUS_SDK_SHA))
        self.assertFalse(has_expected_sdk_gitlink(stale, TINYBUS_SDK_SHA))
        self.assertFalse(has_expected_sdk_gitlink("", TINYBUS_SDK_SHA))
        self.assertFalse(
            has_expected_sdk_gitlink(
                f"100644 blob {TINYBUS_SDK_SHA}\tvendor/tinybus", TINYBUS_SDK_SHA
            )
        )

    def test_a_dispatch_names_only_a_tag_and_no_reviewed_epic_path_remains(self):
        # Releases come only from official main, so there is no mode to choose.
        inputs = self.workflow.split("    inputs:\n", 1)[1].split("  push:\n", 1)[0]
        self.assertEqual(re.findall(r"(?m)^      (\w+):$", inputs), ["tag"])
        self.assertNotIn("reviewed-epic", self.workflow)
        self.assertNotIn("reviewed_pr", self.workflow)
        self.assertNotIn("release_mode", self.workflow)
        self.assertEqual(self.workflow.count("uses: tinyhumansai/.github/"), 1)

    def test_resolve_script_stops_at_the_end_of_its_run_block(self):
        workflow = (
            "      - id: tag\n        run: |\n          echo resolve\n"
            "  later:\n    steps:\n      - run: |\n          echo later\n"
        )
        self.assertEqual(resolve_script(workflow).strip(), "echo resolve")

    def test_an_epic_dispatch_is_refused_a_reviewed_release(self):
        # The old inputs are ignored: the resolver only names the tag, and the
        # one release job is the ordinary module release.
        result, output = self.run_resolve(
            "refs/heads/epic/8056-modules-wiring",
            RELEASE_MODE="reviewed-epic",
            REVIEWED_PR="83",
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(output, "tag=v0.5.2\nversion=0.5.2\n")
        jobs = re.findall(r"(?m)^  ([\w-]+):$", self.workflow.split("\njobs:\n", 1)[1])
        self.assertEqual(jobs, ["resolve", "module-release"])

if __name__ == "__main__":
    unittest.main()
