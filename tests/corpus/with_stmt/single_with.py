def read(path: str) -> str:
    with open(path) as f:
        data = f.read()
    return data
