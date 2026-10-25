from enum import StrEnum


class SessionKind(StrEnum):
    BROWSER = "browser"
    CLI = "cli"
    SCRIPT = "script"

    def __str__(self) -> str:
        return str(self.value)
