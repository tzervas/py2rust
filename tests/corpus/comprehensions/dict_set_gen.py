def build(xs):
    d = {x: x * x for x in xs}
    s = {x for x in xs}
    g = (x for x in xs)
    return d
