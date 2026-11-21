"""Interactive widgets for Jupyter, JupyterLab, VS Code notebooks, Colab and Kaggle.

Usage::

    import zindi
    zindi.explore()

You get a search box, filters, a competition picker and buttons to see details, join,
download the data, and look at the leaderboard and your submissions, without writing
any code. Every action is also available as a plain method on :class:`zindi.Client`.
"""

from __future__ import annotations

from typing import Any

from .client import Client
from .errors import SecretCodeRequiredError, ZindiError
from .models import Competition

try:
    import ipywidgets as widgets
    from IPython.display import display
except ImportError as exc:  # pragma: no cover - depends on the environment
    raise ImportError(
        "The notebook explorer needs ipywidgets: pip install 'zindi[notebook]'"
    ) from exc

__all__ = ["Explorer"]


class Explorer:
    """A small competition browser built with ipywidgets."""

    def __init__(self, client: Client) -> None:
        self.client = client
        self._competitions: list[Competition] = []

        self.search = widgets.Text(placeholder="Search competitions…", layout={"width": "260px"})
        self.status = widgets.ToggleButtons(
            options=[("Open", "open"), ("Closed", "closed"), ("All", "all")], value="open"
        )
        self.kind = widgets.Dropdown(
            options=[
                ("Any kind", None),
                ("Competitions", "competition"),
                ("Hackathons", "hackathon"),
            ],
            layout={"width": "150px"},
        )
        self.refresh_button = widgets.Button(
            description="Search", icon="search", button_style="warning"
        )
        self.picker = widgets.Select(options=[], rows=8, layout={"width": "100%"})
        self.secret_code = widgets.Password(placeholder="Secret code (private competitions)")
        self.folder = widgets.Text(value="data", description="Folder", layout={"width": "260px"})

        self.buttons = {
            "details": widgets.Button(description="Details", icon="info-circle"),
            "join": widgets.Button(description="Join", icon="sign-in", button_style="success"),
            "download": widgets.Button(description="Download data", icon="download"),
            "leaderboard": widgets.Button(description="Leaderboard", icon="trophy"),
            "submissions": widgets.Button(description="My submissions", icon="list"),
            "use": widgets.Button(description="Use in this notebook", icon="thumb-tack"),
        }
        self.output = widgets.Output(layout={"border": "1px solid #ddd", "padding": "6px"})

        self.refresh_button.on_click(lambda _b: self.refresh())
        for name, button in self.buttons.items():
            button.on_click(lambda _b, action=name: self._run(action))

        self.widget = widgets.VBox(
            [
                widgets.HTML("<h3 style='margin:0'>Zindi explorer</h3>"),
                widgets.HBox([self.search, self.kind, self.refresh_button]),
                self.status,
                self.picker,
                widgets.HBox(list(self.buttons.values())[:3]),
                widgets.HBox([*list(self.buttons.values())[3:], self.folder]),
                self.secret_code,
                self.output,
            ]
        )

    # -- behaviour ---------------------------------------------------------------------------

    @property
    def selected(self) -> Competition | None:
        index = self.picker.index
        if index is None or not 0 <= index < len(self._competitions):
            return None
        return self._competitions[index]

    def refresh(self) -> None:
        self.output.clear_output()
        with self.output:
            try:
                self._competitions = list(
                    self.client.competitions(
                        status=self.status.value,
                        kind=self.kind.value,
                        search=self.search.value or None,
                        limit=100,
                    )
                )
            except ZindiError as exc:
                print(f"Could not load competitions: {exc}")
                return
            self.picker.options = [
                f"{'🟢' if c.is_open else '⚪'} {c.title}  ·  {c.reward or ''}"
                for c in self._competitions
            ]
            print(f"{len(self._competitions)} competition(s). Pick one, then choose an action.")

    def _run(self, action: str) -> None:
        comp = self.selected
        self.output.clear_output()
        with self.output:
            if comp is None:
                print("Select a competition first.")
                return
            try:
                self._act(action, comp)
            except SecretCodeRequiredError:
                print("This competition is private: type its secret code above, then click Join.")
            except ZindiError as exc:
                print(f"Error: {exc}")

    def _act(self, action: str, comp: Competition) -> None:
        cid = comp.id
        if action == "details":
            display(self.client.competition(cid))
        elif action == "join":
            joined = self.client.join(cid, secret_code=self.secret_code.value or None)
            print("Joined! Good luck 🍀" if joined else "You were already in.")
        elif action == "download":
            paths = self.client.download(cid, self.folder.value or ".")
            print("Downloaded:", *[f"  {p}" for p in paths], sep="\n")
        elif action == "leaderboard":
            display(self.client.leaderboard(cid, limit=50))
        elif action == "submissions":
            display(self.client.submissions(cid))
        elif action == "use":
            self.client.use(cid)
            print(f"Default competition set to {cid}. Methods now work without an id, e.g.")
            print("  client.submit('submission.csv', comment='...')")

    def show(self) -> Any:
        self.refresh()
        display(self.widget)
        return self
