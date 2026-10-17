from enum import StrEnum


class ReadinessState(StrEnum):
    NEEDS_WORK = "needs_work"
    READY = "ready"
    SUGGESTED = "suggested"

    def __str__(self) -> str:
        return str(self.value)
