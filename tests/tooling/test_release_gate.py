# This Source Code Form is subject to the terms of the Mozilla Public
# License, v. 2.0. If a copy of the MPL was not distributed with this
# file, You can obtain one at https://mozilla.org/MPL/2.0/.
# SPDX-License-Identifier: MPL-2.0

import copy
import importlib.util
from pathlib import Path
import unittest
from unittest import mock

SPEC = importlib.util.spec_from_file_location("release_gate", Path(__file__).resolve().parents[2] / "tools/release_gate.py")
gate = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(gate)


class ReleaseGateTests(unittest.TestCase):
    def setUp(self):
        self.commit = "a" * 40
        self.run = dict(id=1, run_attempt=1, head_sha=self.commit, head_branch="master", event="push", status="completed", conclusion="success")
        self.jobs = [dict(name=name, head_sha=self.commit, run_id=1, status="completed", conclusion="success") for name in gate.REQUIRED_JOBS]

    def test_accepts_exact_commit_with_every_required_job(self):
        gate.validate_run(self.commit, self.run, self.jobs)

    def test_rejects_missing_duplicate_and_unsuccessful_jobs(self):
        for jobs in [self.jobs[1:], [*self.jobs, self.jobs[0]]]:
            with self.assertRaises(ValueError):
                gate.validate_run(self.commit, self.run, jobs)
        for field, value in [("conclusion", "failure"), ("conclusion", "cancelled"), ("conclusion", "skipped"), ("conclusion", None), ("status", "in_progress"), ("head_sha", "b" * 40), ("run_id", 2)]:
            jobs = copy.deepcopy(self.jobs)
            jobs[0][field] = value
            with self.subTest(field=field, value=value), self.assertRaises(ValueError):
                gate.validate_run(self.commit, self.run, jobs)

    def test_rejects_wrong_commit_event_or_failed_run(self):
        for field, value in [("head_sha", "b" * 40), ("head_branch", "feature"), ("event", "pull_request"), ("conclusion", "failure"), ("status", "in_progress")]:
            run = {**self.run, field: value}
            with self.assertRaises(ValueError):
                gate.validate_run(self.commit, run, self.jobs)

    def test_master_advancing_during_verification_fails(self):
        fetch = mock.Mock(side_effect=[
            {"object": {"sha": self.commit}}, {"workflow_runs": [self.run]},
            {"jobs": self.jobs}, {"object": {"sha": "b" * 40}},
        ])
        with self.assertRaisesRegex(ValueError, "advanced during"):
            gate.check(self.commit, fetch)

    def test_new_failed_run_cannot_fall_back_to_old_success(self):
        failed = {**self.run, "id": 2, "conclusion": "failure"}
        fetch = mock.Mock(side_effect=[
            {"object": {"sha": self.commit}}, {"workflow_runs": [self.run, failed]}, {"jobs": []},
        ])
        with self.assertRaisesRegex(ValueError, "not successful"):
            gate.check(self.commit, fetch)
