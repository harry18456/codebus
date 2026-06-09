"""Simplified web auth module — fixture for v3-app-quiz spike.

Not a real implementation; only the wiki/ pages are spike targets.
The agent should NEVER read this file during quiz workflow.
"""

import jwt
import datetime
from functools import wraps

SECRET = "fixture-secret-do-not-use"
EXPIRY_HOURS = 24


def issue_token(user_id: int) -> str:
    payload = {
        "sub": user_id,
        "iat": datetime.datetime.utcnow(),
        "exp": datetime.datetime.utcnow() + datetime.timedelta(hours=EXPIRY_HOURS),
    }
    return jwt.encode(payload, SECRET, algorithm="HS256")


def verify_token(token: str) -> dict | None:
    try:
        return jwt.decode(token, SECRET, algorithms=["HS256"])
    except jwt.InvalidTokenError:
        return None


def auth_middleware(handler):
    @wraps(handler)
    def wrapped(request, *args, **kwargs):
        token = request.headers.get("Authorization", "").removeprefix("Bearer ")
        claims = verify_token(token)
        if not claims:
            return {"status": 401, "body": "unauthorized"}
        request.user_id = claims["sub"]
        return handler(request, *args, **kwargs)
    return wrapped
