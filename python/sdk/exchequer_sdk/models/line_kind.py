from enum import StrEnum


class LineKind(StrEnum):
    CLAIM = "claim"
    CLEARING = "clearing"
    IN_TRANSIT = "in_transit"
    PAYMENT = "payment"

    def __str__(self) -> str:
        return str(self.value)
