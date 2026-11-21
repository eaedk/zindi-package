# Command line

```text
zindi [COMMAND] [ARGS] [OPTIONS]
zindi COMMAND --help        # help for one command
```

Commands that work on a competition take its id as an argument. Run `zindi use <id>` once
in your project folder and you can leave it out everywhere (the id is stored in
`zindi.toml`; the `ZINDI_COMPETITION` environment variable works too).

## Account

| Command | What it does |
|---|---|
| `zindi login [-u USER] [--password-stdin] [--token T]` | Log in and save the token |
| `zindi logout` | Delete the saved token |
| `zindi whoami` | Your profile and where the credentials come from |
| `zindi user USERNAME` | Someone's public profile |

## Competitions

| Command | What it does |
|---|---|
| `zindi competitions [--all] [--status S] [--kind K] [--reward R] [-s TEXT] [-n N]` | List competitions (open ones by default) |
| `zindi show [ID] [--rules]` | Details, data files, metric, daily limit |
| `zindi use ID` | Remember a competition for this folder |
| `zindi join [ID] [--code CODE]` | Join (asks for the code of private competitions) |
| `zindi download [ID] [-o DIR] [-f FILE]... [--unzip] [--force]` | Download data (resumable, skips what you have) |
| `zindi discussions [ID] [-n N]` | Latest discussion threads |

## Submissions and ranking

| Command | What it does |
|---|---|
| `zindi submit FILE [-c ID] [-m COMMENT] [--wait]` | Upload a submission; `--wait` prints the score |
| `zindi submissions [ID] [-n N]` | Your submissions with status and scores |
| `zindi limits [ID]` | Submissions left today |
| `zindi leaderboard [ID] [-n N]` | The leaderboard, your row highlighted |
| `zindi rank [ID]` | Your rank |

## Teams

| Command | What it does |
|---|---|
| `zindi team show [ID]` | Your team, members and pending invitations |
| `zindi team create NAME [-i USER]... [-c ID]` | Create a team and invite people |
| `zindi team invite USER... [-c ID]` | Invite more people |
| `zindi team disband [ID] [--yes]` | Disband your team |

## Diagnostics

`zindi doctor` prints your versions, OS, environment (terminal, Jupyter, Colab, Kaggle),
config paths, API reachability and login status. Paste it in bug reports.

## Scripting

Data commands accept `--json` and print JSON to standard output (messages go to standard
error), so you can pipe them anywhere:

```bash
zindi leaderboard --json | jq '.[0]'
zindi competitions --all --json > competitions.json
```

Exit codes: `0` success, `1` error, `2` authentication problem, `130` interrupted.

Tab completion for bash, zsh, fish and PowerShell: `zindi --install-completion`.
