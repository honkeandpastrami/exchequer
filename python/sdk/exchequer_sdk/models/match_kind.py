from enum import StrEnum


class MatchKind(StrEnum):
    AMOUNT = "amount"
    BANK_ID = "bank_id"
    TRANSFER = "transfer"

    def __str__(self) -> str:
        return str(self.value)
