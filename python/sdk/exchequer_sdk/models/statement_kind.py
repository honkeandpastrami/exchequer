from enum import StrEnum


class StatementKind(StrEnum):
    BANK = "bank"
    CREDIT_CARD = "credit_card"

    def __str__(self) -> str:
        return str(self.value)
