"""Allow ``python -m zindi`` (handy when the ``zindi`` script is not on PATH, e.g. on Windows)."""

from .cli import main

if __name__ == "__main__":
    main()
