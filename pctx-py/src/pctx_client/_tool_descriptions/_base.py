"""Shared utilities for tool descriptions."""

# Detect if search is available
try:
    from bm25s import BM25  # noqa: F401
    from Stemmer import Stemmer  # noqa: F401

    HAS_SEARCH = True
except ImportError:
    HAS_SEARCH = False
