"""Where the server is and how to authenticate: `EXCHEQUER_URL` and
`EXCHEQUER_TOKEN_FILE`, else `~/.config/exchequer/config.toml`. The token
lives in a file of its own and is never printed."""

from __future__ import annotations

import os
import tomllib
from dataclasses import dataclass
from pathlib import Path

CONFIG = Path("~/.config/exchequer/config.toml")


class ConfigError(RuntimeError):
    pass


@dataclass(frozen=True)
class Config:
    url: str
    token_file: Path

    def token(self) -> str:
        try:
            token = self.token_file.read_text().strip()
        except OSError as e:
            raise ConfigError(f"can't read the token file {self.token_file}: {e.strerror}") from e
        if not token:
            raise ConfigError(f"the token file {self.token_file} is empty")
        return token


def load(env: dict[str, str] | None = None, path: Path = CONFIG) -> Config:
    env = dict(os.environ) if env is None else env
    raw: dict[str, object] = {}
    file = path.expanduser()
    if file.exists():
        try:
            raw = tomllib.loads(file.read_text())
        except tomllib.TOMLDecodeError as e:
            raise ConfigError(f"{file}: {e}") from e
    url = env.get("EXCHEQUER_URL") or raw.get("url")
    token_file = env.get("EXCHEQUER_TOKEN_FILE") or raw.get("token_file")
    if not isinstance(url, str) or not isinstance(token_file, str) or not url or not token_file:
        raise ConfigError(
            f"set EXCHEQUER_URL and EXCHEQUER_TOKEN_FILE, or url and token_file in {path}"
        )
    return Config(url.rstrip("/"), Path(token_file).expanduser())
