def describe(n: int) -> str:
    match n:
        case 0:
            return "zero"
        case 1 | 2:
            return "small"
        case _:
            return "other"
