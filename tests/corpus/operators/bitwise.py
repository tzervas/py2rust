def bits(a: int, b: int) -> int:
    return (a & b) | (a ^ b) << 1 >> 1
