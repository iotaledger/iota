#!/usr/bin/env python3
# Copyright (c) 2026 IOTA Stiftung
# SPDX-License-Identifier: Apache-2.0
#
# Run with: python3 -m unittest discover scripts/release_notes

import unittest

import release_notes as rn

REPO = rn.GH_REPO

NOTE = """
### Release Notes

- [x] CLI: note from PR %d
"""


def pull_request(number, branch, merged_at, repo=REPO, body=""):
    return {
        "number": number,
        "body": body,
        "merged_at": merged_at,
        "head": {"ref": branch, "repo": {"full_name": repo} if repo else None},
    }


# A feature branch `feat/big` that collected PRs 101, 102 and 105, where 102 is
# itself a feature branch that collected 103. 104 is an unrelated PR that
# targeted an earlier branch of the same name, and 105 was closed unmerged.
PULLS = {
    100: pull_request(100, "feat/big", "2026-06-01T00:00:00Z", body="no notes"),
    101: pull_request(101, "sub/one", "2026-05-10T00:00:00Z", body=NOTE % 101),
    102: pull_request(102, "feat/big-sub", "2026-05-20T00:00:00Z", body=NOTE % 102),
    103: pull_request(103, "sub/three", "2026-05-15T00:00:00Z", body=NOTE % 103),
    104: pull_request(104, "sub/reused", "2026-04-01T00:00:00Z", body=NOTE % 104),
    105: pull_request(105, "sub/abandoned", None, body=NOTE % 105),
    200: pull_request(200, "fix/plain", "2026-06-02T00:00:00Z", body=NOTE % 200),
    300: pull_request(
        300, "fix/fork", "2026-06-03T00:00:00Z", repo="contributor/iota"
    ),
}

BY_BASE = {
    "feat/big": [101, 102, 104, 105],
    "feat/big-sub": [103],
}

# First commit of each branch, bounding how far back its PRs are looked for.
FIRST_COMMITS = {
    100: ("2026-05-01T00:00:00Z", "2026-05-02T00:00:00Z"),
    102: ("2026-05-12T00:00:00Z", "2026-05-12T00:00:00Z"),
}


class FindSubPrs(unittest.TestCase):
    def setUp(self):
        self.calls = []
        self.real_api = rn.github_api
        rn.github_api = self.fake_api
        rn.PR_CACHE.clear()
        self.addCleanup(rn.PR_CACHE.clear)
        self.addCleanup(setattr, rn, "github_api", self.real_api)

    def fake_api(self, path, params=None):
        self.calls.append(path)
        if path == "/pulls":
            if params["page"] > 1:
                return []
            return [PULLS[n] for n in BY_BASE.get(params["base"], [])]
        if path.endswith("/commits"):
            dates = FIRST_COMMITS.get(int(path.split("/")[2]))
            if not dates:
                return []
            author, committer = dates
            return [
                {"commit": {"author": {"date": author},
                            "committer": {"date": committer}}}
            ]
        if path.startswith("/pulls/"):
            return PULLS[int(path.split("/")[2])]
        raise AssertionError(f"unexpected request {path}")

    def test_expands_a_feature_branch_recursively(self):
        self.assertEqual(rn.find_sub_prs(100), ["101", "102", "103"])

    def test_ignores_prs_merged_before_the_branch_existed(self):
        self.assertNotIn("104", rn.find_sub_prs(100))

    def test_ignores_prs_that_were_never_merged(self):
        self.assertNotIn("105", rn.find_sub_prs(100))

    def test_a_plain_pr_has_no_sub_prs(self):
        self.assertEqual(rn.find_sub_prs(200), [])

    def test_a_fork_branch_is_not_looked_up(self):
        self.assertEqual(rn.find_sub_prs(300), [])
        self.assertNotIn("/pulls", self.calls)

    def test_skips_prs_already_accounted_for(self):
        self.assertEqual(rn.find_sub_prs(100, {"102"}), ["101"])

    def test_reuses_the_payloads_it_has_already_listed(self):
        rn.find_sub_prs(100)
        # Only the PR expansion starts from is fetched on its own: the rest were
        # listed by base branch, so they are read back out of the cache.
        fetched = [
            c
            for c in self.calls
            if c.startswith("/pulls/") and not c.endswith("/commits")
        ]
        self.assertEqual(fetched, ["/pulls/100"])
        self.assertEqual(rn.extract_notes_from_pr(101), NOTE % 101)
        self.assertNotIn("/pulls/101", self.calls)

    def test_notes_of_an_expanded_pr_are_readable(self):
        notes = dict(rn.extract_notes("101", set(), True, set(), False)[1])
        self.assertEqual(notes["CLI"], rn.Note(checked=True, note="note from PR 101"))


if __name__ == "__main__":
    unittest.main()
