from enum import StrEnum


class ReviewState(StrEnum):
    NEEDS_YOU = "needs_you"
    NOT_ADDED = "not_added"
    READY = "ready"
    TO_REVIEW = "to_review"

    def __str__(self) -> str:
        return str(self.value)
