from enum import StrEnum


class LineKind(StrEnum):
    CLAIM = "claim"
    PAYMENT = "payment"

    def __str__(self) -> str:
        return str(self.value)
