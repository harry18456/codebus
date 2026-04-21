"""Fixture file — contains **synthetic** AWS credential pattern.

The access key below is the public AWS-documented dummy value and has no
access to any real account; it exists purely to exercise the sanitizer's
detect-secrets rule path.  See AWS documentation for the canonical
example value.
"""

AWS_ACCESS_KEY_ID = "AKIAIOSFODNN7EXAMPLE"
AWS_SECRET_ACCESS_KEY = "wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY"
