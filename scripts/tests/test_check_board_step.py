"""We exercise the generated sphere report check without running geometry imports."""

import json
import math
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest


class SphereReportTests(unittest.TestCase):
    def check_report(self, report):
        script = Path(__file__).resolve().parents[1] / "check-board-step.sh"
        sphere_section = script.read_text().split('echo "== generated sphere"', 1)[1]
        validator = sphere_section.split("<<'PY' || failed=1\n", 1)[1].split("\nPY\n", 1)[0]
        with tempfile.TemporaryDirectory() as folder:
            report_path = Path(folder) / "sphere.import.json"
            report_path.write_text(json.dumps(report))
            result = subprocess.run(
                [sys.executable, "-c", validator, str(report_path)],
                capture_output=True,
                text=True,
                check=False,
            )
        return result.returncode, result.stdout

    def test_wrong_body_counts_have_an_error_with_the_count(self):
        for report, count in [({}, 0), ({"bodies": []}, 0), ({"bodies": [{}, {}]}, 2)]:
            with self.subTest(count=count, report=report):
                code, output = self.check_report(report)
                self.assertEqual(code, 1)
                self.assertIn(
                    f"::error::SolidCraft on the generated sphere: {count} bodies, expected 1",
                    output,
                )

    def test_one_sphere_with_the_derived_volume_and_box_passes(self):
        # We use r = 1.25: diameter 2r = 2.5 and volume = 4/3 pi r cubed.
        code, output = self.check_report({
            "bodies": [{
                "volume_mm3": 4 / 3 * math.pi * 1.25 ** 3,
                "bbox": {"min": [-1.25] * 3, "max": [1.25] * 3},
            }],
        })
        self.assertEqual(code, 0)
        self.assertIn("volume and box as expected", output)


if __name__ == "__main__":
    unittest.main()
