"""Rendering of models as rich tables (terminal) and HTML (Jupyter, Colab, VS Code)."""

from __future__ import annotations

import html
from datetime import datetime
from typing import TYPE_CHECKING, Any

from rich import box
from rich.console import Console, ConsoleOptions, Group, JustifyMethod, RenderableType, RenderResult
from rich.panel import Panel
from rich.table import Table
from rich.text import Text

if TYPE_CHECKING:
    from .models import Competition, ResultList, _Model

_ACCENT = "#f5a623"  # Zindi-ish amber
_NUMERIC = frozenset(
    {"Rank", "Score", "Subs", "Participants", "Public", "Private", "Size", "Replies"}
)
_WRAPPING = frozenset({"Title", "Description", "Comment", "Members"})


def _value(model: _Model, attr: str) -> Any:
    value = getattr(model, attr, None)
    if isinstance(value, float):
        return f"{value:.6g}"
    if isinstance(value, datetime):
        return value.strftime("%Y-%m-%d %H:%M")
    if isinstance(value, bool):
        return "yes" if value else "no"
    if isinstance(value, list):
        return ", ".join(str(v) for v in value)
    return "" if value is None else str(value)


def _style_for(model: _Model, attr: str) -> str | None:
    if attr == "status_label":
        label = getattr(model, attr, "")
        return {"failed": "red", "scored": "green"}.get(label, "yellow")
    if attr == "status":
        return "green" if getattr(model, attr, "") == "open" else "dim"
    return None


# -- terminal --------------------------------------------------------------------------------


def rich_table(rows: ResultList[Any], *, highlight: str | None = None) -> RenderableType:
    """A table that fits the terminal: long text columns are dropped when space is short.

    ``highlight`` is a username whose leaderboard row is emphasised.
    """
    return _FitTable(rows, highlight)


class _FitTable:
    def __init__(self, rows: ResultList[Any], highlight: str | None) -> None:
        self.rows = rows
        self.highlight = highlight

    def __rich_console__(self, console: Console, options: ConsoleOptions) -> RenderResult:
        if not self.rows:
            label = f"{self.rows.title}: " if self.rows.title else ""
            yield Text(f"{label}nothing to show yet.", style="dim")
            return
        room = self._room_for_text(options.max_width)
        yield _build_table(self.rows, self.highlight, text_width=room if room >= 16 else None)

    def _room_for_text(self, width: int) -> int:
        """Characters left for each long-text column once the other columns are laid out."""
        if not self.rows:
            return width
        fixed, text_columns = 4, 0
        for header, attr in type(self.rows[0]).columns:
            if header in _WRAPPING:
                text_columns += 1
                continue
            fixed += max(len(header), *(len(_value(r, attr)) for r in self.rows)) + 4
        return (width - fixed) // max(text_columns, 1) - 3


def _build_table(rows: ResultList[Any], highlight: str | None, *, text_width: int | None) -> Table:
    """Build the table; long-text columns get ``text_width`` characters, or are left out."""
    columns = type(rows[0]).columns if rows else ()
    if text_width is None and len(columns) > 2:
        columns = tuple(c for c in columns if c[0] not in _WRAPPING) or columns
    caption = None
    if rows.total is not None and rows.total > len(rows):
        caption = f"showing {len(rows)} of {rows.total}"
    table = Table(
        title=rows.title or None,
        caption=caption,
        box=box.SIMPLE_HEAVY,
        header_style=f"bold {_ACCENT}",
        title_style="bold",
    )
    if not rows:
        table.add_column("No results", style="dim")
        return table
    for header, attr in columns:
        justify: JustifyMethod = "right" if header in _NUMERIC else "left"
        if header in _WRAPPING:
            table.add_column(
                header, no_wrap=True, overflow="ellipsis", max_width=min(text_width or 56, 56)
            )
        elif attr == "id":  # ids are meant to be copied: never truncate them
            table.add_column(
                header, no_wrap=True, min_width=max(len(_value(r, attr)) for r in rows)
            )
        else:
            table.add_column(header, justify=justify, no_wrap=True)
    for row in rows:
        mine = bool(highlight) and getattr(row, "includes", lambda _u: False)(highlight)
        cells = [Text(_value(row, attr), style=_style_for(row, attr) or "") for _, attr in columns]
        table.add_row(*cells, style=f"bold {_ACCENT}" if mine else None)
    return table


def rich_record(model: _Model) -> RenderableType:
    from .models import Competition

    if isinstance(model, Competition):
        return rich_competition(model)
    table = Table.grid(padding=(0, 2))
    table.add_column(style=f"bold {_ACCENT}")
    table.add_column()
    for header, attr in type(model).columns:
        table.add_row(header, _value(model, attr))
    return table


def rich_competition(comp: Competition) -> RenderableType:
    facts = Table.grid(padding=(0, 2))
    facts.add_column(style=f"bold {_ACCENT}")
    facts.add_column()
    rows = [
        ("Id", comp.id),
        ("Organizer", comp.organization or ""),
        ("Kind", comp.kind or ""),
        ("Reward", comp.reward or ""),
        ("Status", comp.status),
        ("Ends", comp.end_date),
        ("Participants", str(comp.participants or "")),
        ("Metric", comp.metric or ""),
        ("Team size", str(comp.max_team_size or "")),
        ("Daily limit", str(comp.daily_submission_limit or "")),
        ("Private", "yes (secret code)" if comp.secret_code_required else ""),
        ("Joined", "yes" if comp.is_participant else ""),
        ("Link", comp.url),
    ]
    for key, value in rows:
        if value:
            facts.add_row(key, value)
    parts: list[RenderableType] = [facts]
    if comp.data_files:
        from .models import ResultList

        files: ResultList[Any] = ResultList(comp.data_files, title="Data files")
        parts.append(rich_table(files))
    return Panel(Group(*parts), title=f"[bold]{comp.title}[/]", border_style=_ACCENT, expand=False)


# -- HTML ------------------------------------------------------------------------------------

_CSS = (
    "<style>"
    ".zindi-table{border-collapse:collapse;font-size:13px;margin:4px 0}"
    ".zindi-table th{text-align:left;padding:4px 10px;border-bottom:2px solid #f5a623}"
    ".zindi-table td{padding:3px 10px;border-bottom:1px solid rgba(127,127,127,.25)}"
    ".zindi-table tr.zindi-me td{font-weight:600;background:rgba(245,166,35,.15)}"
    ".zindi-caption{font-size:12px;opacity:.7;margin:2px 0 8px}"
    ".zindi-card{border:1px solid rgba(127,127,127,.35);border-left:4px solid #f5a623;"
    "border-radius:6px;padding:10px 14px;max-width:760px}"
    ".zindi-card h3{margin:0 0 6px}"
    "</style>"
)


def _esc(value: Any) -> str:
    return html.escape(str(value))


def html_table(rows: ResultList[Any], *, highlight: str | None = None) -> str:
    if not rows:
        return f"{_CSS}<p class='zindi-caption'>No results.</p>"
    columns = type(rows[0]).columns
    head = "".join(f"<th>{_esc(h)}</th>" for h, _ in columns)
    body = []
    for row in rows:
        mine = bool(highlight) and getattr(row, "includes", lambda _u: False)(highlight)
        cells = []
        for _, attr in columns:
            value = _value(row, attr)
            if attr == "id" and hasattr(row, "url"):
                value_html = f"<a href='{_esc(row.url)}' target='_blank'>{_esc(value)}</a>"
            else:
                value_html = _esc(value)
            cells.append(f"<td>{value_html}</td>")
        body.append(f"<tr class='{'zindi-me' if mine else ''}'>{''.join(cells)}</tr>")
    title = f"<b>{_esc(rows.title)}</b>" if rows.title else ""
    caption = ""
    if rows.total is not None and rows.total > len(rows):
        caption = f"<div class='zindi-caption'>showing {len(rows)} of {rows.total}</div>"
    return (
        f"{_CSS}{title}<table class='zindi-table'><thead><tr>{head}</tr></thead>"
        f"<tbody>{''.join(body)}</tbody></table>{caption}"
    )


def html_record(model: _Model) -> str:
    from .models import Competition, ResultList

    if isinstance(model, Competition):
        comp = model
        facts = [
            ("Organizer", comp.organization),
            ("Reward", comp.reward),
            ("Status", comp.status),
            ("Ends", comp.end_date),
            ("Participants", comp.participants),
            ("Metric", comp.metric),
            ("Daily limit", comp.daily_submission_limit),
        ]
        items = " · ".join(f"<b>{_esc(k)}</b>: {_esc(v)}" for k, v in facts if v)
        files = html_table(ResultList(comp.data_files)) if comp.data_files else ""
        return (
            f"{_CSS}<div class='zindi-card'><h3><a href='{_esc(comp.url)}' target='_blank'>"
            f"{_esc(comp.title)}</a></h3><div class='zindi-caption'>{_esc(comp.id)}</div>"
            f"<div>{items}</div>{files}</div>"
        )
    rows = "".join(
        f"<tr><th>{_esc(h)}</th><td>{_esc(_value(model, a))}</td></tr>"
        for h, a in type(model).columns
    )
    return f"{_CSS}<table class='zindi-table'>{rows}</table>"
