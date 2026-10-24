from enum import StrEnum


class TokenKind(StrEnum):
    CLI = "cli"
    SCRIPT = "script"
    SESSION = "session"

    def __str__(self) -> str:
        return str(self.value)
