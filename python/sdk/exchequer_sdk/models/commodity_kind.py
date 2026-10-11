from enum import StrEnum


class CommodityKind(StrEnum):
    CRYPTO = "crypto"
    FIAT = "fiat"
    FUND = "fund"
    OTHER = "other"
    SECURITY = "security"

    def __str__(self) -> str:
        return str(self.value)
