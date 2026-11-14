#!/usr/bin/env python3
"""Run the Conflux evidence integrity guards."""
from unittest_report import run_guards

if __name__ == '__main__':
    raise SystemExit(run_guards('test_conflux_link.py', 6))
