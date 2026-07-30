def counter(n: int):
    i = 0
    while i < n:
        yield i
        i = i + 1
