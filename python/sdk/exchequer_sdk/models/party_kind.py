from enum import StrEnum


class PartyKind(StrEnum):
    BUSINESS = "business"
    HOUSEHOLD = "household"
    PERSON = "person"

    def __str__(self) -> str:
        return str(self.value)
