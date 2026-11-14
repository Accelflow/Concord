#!/usr/bin/env python3
"""Run the independent selection assurance guards."""
from unittest_report import run_guards

if __name__ == '__main__':
    raise SystemExit(run_guards('test_selection_assurance.py', 11))
