from enum import StrEnum


class BatchKind(StrEnum):
    FILE = "file"
    MANUAL = "manual"
    MIGRATION = "migration"

    def __str__(self) -> str:
        return str(self.value)
