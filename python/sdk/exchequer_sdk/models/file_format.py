from enum import StrEnum


class FileFormat(StrEnum):
    CSV = "csv"
    OFX = "ofx"

    def __str__(self) -> str:
        return str(self.value)
