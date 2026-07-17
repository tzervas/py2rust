import os
from typing import Any


def typed_add(a: int, b: int) -> int:
    return a


def untyped(x):
    return x


class Point:
    pass


def risky(n: int) -> int:
    try:
        return n
    except Exception:
        return -1


f = lambda z: z


def decorated():
    pass


@decorated
def with_deco(y: int) -> int:
    return y


def meta():
    eval("1 + 1")
