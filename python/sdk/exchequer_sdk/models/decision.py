from enum import StrEnum


class Decision(StrEnum):
    MATCH = "match"
    NEW = "new"
    SKIP = "skip"

    def __str__(self) -> str:
        return str(self.value)
