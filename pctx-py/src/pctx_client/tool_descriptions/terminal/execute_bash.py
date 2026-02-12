"""Terminal-style description for execute_bash tool."""

DESCRIPTION = """Execute bash commands in the SDK filesystem.

You're working in an in-memory virtual filesystem mounted at /sdk/ containing:
- README.md - Overview of available functions by namespace
- {Namespace}/ directories - One per namespace, containing .d.ts type definition files

Current directory: /sdk/

Standard bash utilities available: ls, cat, grep, find, etc.
This filesystem is read-only - used for exploring the SDK before writing code."""
