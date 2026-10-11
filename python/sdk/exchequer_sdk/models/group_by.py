from enum import StrEnum


class GroupBy(StrEnum):
    ACCOUNT = "account"
    CATEGORY = "category"
    COUNTERPARTY = "counterparty"
    KIND = "kind"
    OWNER = "owner"
    PURPOSE = "purpose"
    TAG = "tag"

    def __str__(self) -> str:
        return str(self.value)
