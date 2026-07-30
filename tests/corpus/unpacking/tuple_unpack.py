def swap(a: int, b: int):
    a, b = b, a
    return a, b


def star_unpack(xs):
    first, *rest = xs
    return first
