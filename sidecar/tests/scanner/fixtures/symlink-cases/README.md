# symlink-cases

POSIX-only fixture. Windows runners should skip tests that depend on this
fixture (symlink creation requires elevated privileges).

Layout:

```
symlink-cases/
  README.md
  outside_target.txt        <- out-of-workspace resolve target
  workspace/                <- the scanner's `workspace_root` argument
    real.py
    link.py     -> real.py                    (in-workspace)
    escape.lnk  -> ../outside_target.txt      (out-of-workspace)
```

The two symlinks under `workspace/` are NOT checked into git (git stores
symlinks inconsistently across platforms). Tests materialize them at runtime
via `Path.symlink_to(...)`.
