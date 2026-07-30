class Box:
    def __init__(self, v: int):
        self._v = v

    @property
    def value(self) -> int:
        return self._v

    def __repr__(self) -> str:
        return "Box"
