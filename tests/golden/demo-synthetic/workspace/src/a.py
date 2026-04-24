"""Golden fixture — dummy file A.

The golden replay harness uses this workspace to exercise the Explorer
loop under MockProvider; actual content does not matter because the
scripted MockTools never read these files. The workspace just needs to
exist on disk so ensure_in_workspace + path resolution work.
"""
