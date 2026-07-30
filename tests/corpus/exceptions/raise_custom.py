class MyError(Exception):
    pass


def check(n: int) -> int:
    if n < 0:
        raise MyError("negative")
    return n
