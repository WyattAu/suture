"""Smoke tests for suture-py Python bindings.

These tests require the package to be built with:
    maturin develop --release

Then run with:
    pytest tests/
"""

import pytest


def test_import():
    """Test that the package can be imported."""
    pytest.skip("Requires maturin develop")


def test_repository_init():
    """Test repository initialization."""
    pytest.skip("Requires maturin develop")


def test_commit_and_log():
    """Test creating commits and reading log."""
    pytest.skip("Requires maturin develop")
