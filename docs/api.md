# The Zindi API

Zindi does not publish an API reference. The clients rely on the endpoints used by the
website, described in [`spec/openapi.yaml`](https://github.com/eaedk/zindi-package/blob/main/spec/openapi.yaml).

| Endpoint | Used for | Account |
|---|---|:---:|
| `POST /auth/signin` | Log in | |
| `GET /me` | Your profile | ✔ |
| `GET /users/{username}` | Public profiles | |
| `GET /competitions` | List and search | |
| `GET /competitions/{id}` | Details, data files, rules | |
| `POST /competitions/{id}/participations` | Join | ✔ |
| `GET /competitions/{id}/participations` | Leaderboard | |
| `GET /competitions/{id}/participations/my_participation` | Your rank | ✔ |
| `GET /competitions/{id}/files/{filename}` | Download a data file | ✔ |
| `GET/POST /competitions/{id}/submissions` | List / upload submissions | ✔ |
| `GET /competitions/{id}/submissions/limits` | Submissions left today | ✔ |
| `GET/POST/DELETE /competitions/{id}/my_team` | Team | ✔ |
| `POST /competitions/{id}/my_team/invite` | Invite a teammate | ✔ |
| `GET /competitions/{id}/discussions` | Discussions | |

Conventions: responses are wrapped as `{"data": ..., "meta": ...}`; errors appear as
`{"data": {"errors": ...}}`, sometimes with HTTP 200; the token goes in the `auth-token`
header; lists paginate with `page` (from 0) and `per_page`. The old host
`api.zindi.africa` redirects to `api.zindi.world`.

When Zindi changes something, the weekly *Live API check* workflow opens an issue. Fixing it
starts in `spec/` (see [Contributing](contributing.md)).
