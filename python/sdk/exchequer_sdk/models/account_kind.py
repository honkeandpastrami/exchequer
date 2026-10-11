from enum import StrEnum


class AccountKind(StrEnum):
    BROKERAGE = "brokerage"
    CASH = "cash"
    CHECKING = "checking"
    CREDIT_CARD = "credit_card"
    MORTGAGE = "mortgage"
    OTHER = "other"
    SAVINGS = "savings"

    def __str__(self) -> str:
        return str(self.value)
