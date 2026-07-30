def memo(f):
    return f


@memo
def slow(n: int) -> int:
    return n
