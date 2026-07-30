class Base:
    def __init__(self):
        self.v = 0


class Child(Base):
    def __init__(self):
        super().__init__()
