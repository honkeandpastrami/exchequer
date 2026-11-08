from enum import StrEnum


class RowStatus(StrEnum):
    DUPLICATE = "duplicate"
    NEW = "new"
    NO_ACCOUNT = "no_account"
    POSSIBLE_DUPLICATE = "possible_duplicate"
    TRANSFER_PAIR = "transfer_pair"

    def __str__(self) -> str:
        return str(self.value)
