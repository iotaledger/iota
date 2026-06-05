#!/usr/bin/env python3

# Copyright (c) 2026 IOTA Stiftung
# SPDX-License-Identifier: Apache-2.0

from __future__ import annotations

import runpy
import subprocess
import tempfile
import unittest
from pathlib import Path
from types import SimpleNamespace
from unittest import mock

import experiment_common as ec


class ExperimentCommonTests(unittest.TestCase):
    def test_validator_count_bounds(self) -> None:
        ec.validate_num_validators(2)
        ec.validate_num_validators(30)
        for invalid in (1, 31):
            with self.subTest(invalid=invalid):
                with self.assertRaises(ValueError):
                    ec.validate_num_validators(invalid)

    def test_commit_latency_percentiles_use_histograms(self) -> None:
        queries = ec._commit_latency_queries(90)
        self.assertIn("histogram_quantile(0.5", queries["blk_p50"])
        self.assertIn("histogram_quantile(0.5", queries["txn_p50"])
        self.assertNotIn("_sum", queries["blk_p50"])
        self.assertNotIn("_sum", queries["txn_p50"])
        self.assertNotIn("block_commit", queries["txn_p50"])

    @mock.patch("experiment_common.subprocess.run")
    def test_validator_log_snapshot_names(
        self, run_mock: mock.Mock
    ) -> None:
        run_mock.return_value = subprocess.CompletedProcess([], 0)
        with tempfile.TemporaryDirectory() as tmp:
            log_dir = Path(tmp)
            ec.save_validator_logs(log_dir, 2, prefix="exp")
            ec.save_validator_logs(
                log_dir, 2, prefix="experiment-20260605-120000", latest=False
            )
            self.assertTrue((log_dir / "exp-validator-1-latest.log").exists())
            self.assertTrue(
                (log_dir / "experiment-20260605-120000-validator-2.log").exists()
            )

    def test_requested_iota_spammer_must_exist(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            cfg = SimpleNamespace(
                spammer_enable=True,
                spammer_type="iota-spammer",
                spammer_tps=10,
                spammer_size="10KiB",
                run_duration=60,
                block_measurement_seconds=0,
                log_dir=Path(tmp),
            )
            with mock.patch.object(ec.Path, "home", return_value=Path(tmp)):
                with self.assertRaisesRegex(RuntimeError, "script not found"):
                    ec.start_spammer(cfg)

    def test_host_spammer_cleanup_signals_process_group(self) -> None:
        cfg = SimpleNamespace(
            spammer_enable=True,
            spammer_type="iota-spammer",
        )
        proc = mock.Mock(pid=1234)
        with mock.patch("experiment_common.os.killpg") as killpg:
            ec.stop_spammer(cfg, proc)
        killpg.assert_called_once_with(1234, ec.signal.SIGTERM)
        proc.wait.assert_called_once_with(timeout=10)

    def test_migration_reserves_stable_window_after_setup(self) -> None:
        migration = runpy.run_path(
            Path(__file__).with_name("run-migration-test.py"),
            run_name="migration_test_module",
        )
        config = migration["Config"](num_validators=30)
        self.assertEqual(config.stable_window_seconds, 60)
        self.assertFalse(
            migration["Config"](mode="advanced").block_measurement_enabled()
        )
        monitor = migration["CheckpointMonitor"]
        self.assertIn("histogram_quantile(0.5", monitor._BLK_P50)
        self.assertNotIn("block_commit", monitor._TXN_P50)
        planned_start = 1_000.0 + config.pre_rolling_wait
        with self.assertRaisesRegex(RuntimeError, "stable window does not fit"):
            migration["phase7_wait_fixed"](
                config,
                1_000.0,
                planned_start + 1,
            )


if __name__ == "__main__":
    unittest.main()
