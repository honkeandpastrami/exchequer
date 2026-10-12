from enum import StrEnum


class KindName(StrEnum):
    ADJUSTMENT = "adjustment"
    CONVERSION = "conversion"
    EXPENSE = "expense"
    INCOME = "income"
    TRANSFER = "transfer"

    def __str__(self) -> str:
        return str(self.value)
