def may_fail(x: int) -> int:
    try:
        return x
    except ValueError:
        return 0


raise RuntimeError("top-level")
