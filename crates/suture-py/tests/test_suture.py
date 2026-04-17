import pytest


class TestSutureRepo:
    def test_import(self):
        """Test that the module can be imported."""
        import suture

        assert hasattr(suture, "SutureRepo")

    def test_repo_status_class(self):
        """Test RepoStatus class exists with expected attributes."""
        import suture

        assert hasattr(suture, "RepoStatus")

    def test_log_entry_class(self):
        """Test PyLogEntry class exists."""
        import suture

        assert hasattr(suture, "PyLogEntry")

    def test_hash_bytes_function(self):
        """Test hash_bytes utility function."""
        import suture

        assert hasattr(suture, "hash_bytes")

    def test_is_repo_function(self):
        """Test is_repo utility function."""
        import suture

        assert hasattr(suture, "is_repo")

    def test_worktree_entry_class(self):
        """Test PyWorktreeEntry class exists."""
        import suture

        assert hasattr(suture, "PyWorktreeEntry")

    def test_blame_entry_class(self):
        """Test PyBlameEntry class exists."""
        import suture

        assert hasattr(suture, "PyBlameEntry")

    def test_diff_entry_class(self):
        """Test PyDiffEntry class exists."""
        import suture

        assert hasattr(suture, "PyDiffEntry")

    def test_merge_result_class(self):
        """Test PyMergeResult class exists."""
        import suture

        assert hasattr(suture, "PyMergeResult")

    def test_conflict_info_class(self):
        """Test PyConflictInfo class exists."""
        import suture

        assert hasattr(suture, "PyConflictInfo")

    def test_rebase_result_class(self):
        """Test PyRebaseResult class exists."""
        import suture

        assert hasattr(suture, "PyRebaseResult")

    def test_gc_result_class(self):
        """Test PyGcResult class exists."""
        import suture

        assert hasattr(suture, "PyGcResult")

    def test_fsck_result_class(self):
        """Test PyFsckResult class exists."""
        import suture

        assert hasattr(suture, "PyFsckResult")

    def test_stash_entry_class(self):
        """Test PyStashEntry class exists."""
        import suture

        assert hasattr(suture, "PyStashEntry")

    def test_all_export(self):
        """Test __all__ is defined and contains expected symbols."""
        import suture

        expected = [
            "SutureRepo",
            "RepoStatus",
            "PyLogEntry",
            "PyDiffEntry",
            "PyMergeResult",
            "PyConflictInfo",
            "PyRebaseResult",
            "PyGcResult",
            "PyFsckResult",
            "PyStashEntry",
            "PyWorktreeEntry",
            "PyBlameEntry",
            "hash_bytes",
            "is_repo",
        ]
        for name in expected:
            assert name in suture.__all__, f"{name} missing from __all__"
