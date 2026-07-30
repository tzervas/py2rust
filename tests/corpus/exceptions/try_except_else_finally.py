def safe_div(a: int, b: int) -> int:
    try:
        r = a // b
    except ZeroDivisionError:
        r = 0
    else:
        r = r + 1
    finally:
        r = r
    return r
