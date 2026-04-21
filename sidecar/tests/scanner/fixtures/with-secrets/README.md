# with-secrets fixture

This fixture backs scanner-sanitizer-orchestration integration tests.

It intentionally ships files whose bodies trigger the built-in sanitizer
rules (email regex + detect-secrets AWS plugin).  All values are
synthetic / publicly documented dummies; none grant access to any real
system.

Expected scan behaviour:

- `contacts.txt` — two emails redacted; `sanitize_stats == {"email": 2}`.
- `config.py` — synthetic AWS credentials redacted by the detect-secrets
  rule path.
- `README.md` — this file; clean, no sanitizer hits.
- `.gitignore` — empty; exists so scanner's ignore pipeline runs against
  a real pathspec file.
