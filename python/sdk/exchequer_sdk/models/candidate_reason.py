from enum import StrEnum


class CandidateReason(StrEnum):
    DATE_BOUNDARY = "date_boundary"
    DUPLICATE_PAIR = "duplicate_pair"
    EQUALS_DIFFERENCE = "equals_difference"
    MULTIPLE_OF_NINE = "multiple_of_nine"
    NEAR_DUPLICATE = "near_duplicate"
    SIGN_FLIPPED = "sign_flipped"
    SUBSET = "subset"
    TRANSPOSITION = "transposition"

    def __str__(self) -> str:
        return str(self.value)
