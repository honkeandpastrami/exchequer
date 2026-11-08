from enum import StrEnum


class Decision(StrEnum):
    ADD = "add"
    ADD_SEPARATELY = "add_separately"
    DONT_ADD = "dont_add"

    def __str__(self) -> str:
        return str(self.value)
