from enum import StrEnum


class Label(StrEnum):
    CATEGORY = "category"
    COUNTERPARTY = "counterparty"
    PURPOSE = "purpose"

    def __str__(self) -> str:
        return str(self.value)
