from enum import StrEnum


class Measure(StrEnum):
    BALANCE = "balance"
    FLOW = "flow"

    def __str__(self) -> str:
        return str(self.value)
