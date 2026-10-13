from enum import StrEnum


class RowStatus(StrEnum):
    DUPLICATE = "duplicate"
    NEAR_DUPLICATE = "near_duplicate"
    NEEDS_ACCOUNT = "needs_account"
    NEW = "new"
    TRANSFER_DUPLICATE = "transfer_duplicate"
    TRANSFER_MATCH = "transfer_match"

    def __str__(self) -> str:
        return str(self.value)
