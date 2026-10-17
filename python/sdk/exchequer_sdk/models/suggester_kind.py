from enum import StrEnum


class SuggesterKind(StrEnum):
    LOCAL = "local"
    MATCH = "match"
    REMOTE = "remote"
    RULE = "rule"

    def __str__(self) -> str:
        return str(self.value)
