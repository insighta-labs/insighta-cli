# insighta

![Rust](https://img.shields.io/badge/Rust-1.85+-orange?logo=rust)
![Clap](https://img.shields.io/badge/Clap-4-blue)
![Platform](https://img.shields.io/badge/Platform-macOS%20%7C%20Linux%20%7C%20Windows-lightgrey)

`insighta` is a globally installable CLI for Insighta Labs+. It authenticates via GitHub OAuth with PKCE, stores credentials locally, and lets you query, create, and export demographic profiles — all from the terminal.

---

## Install

### Automatic (Mac & Linux)

This script will install Rust for you if it's missing and set up your PATH automatically.

```bash
chmod +x install.sh && ./install.sh
insighta --version
```

### Manual

If you already have Rust installed:

```bash
cargo install --path .
insighta --version
```

> Requires Rust 1.85+. After install, `insighta` is available in any directory.

---

## Quick Start

```bash
insighta login
insighta profiles list --country NG --age-group adult
insighta profiles export --gender male --sort-by age --order desc
insighta logout
```

---

## Prerequisites

- Rust 1.85+
- A running instance of `insighta-api` (see the [backend repo](../insighta-api/README.md))
- A GitHub OAuth App with `http://127.0.0.1:<INSIGHTA_CALLBACK_PORT>/callback` registered as a callback URL

### GitHub OAuth App

Create one at **GitHub → Settings → Developer Settings → GitHub Apps → New GitHub App**. Add `http://127.0.0.1:8182/callback` as one of the callback URLs (GitHub Apps support up to 10). Copy the Client ID and Client Secret into the backend's `.env` as `GITHUB_CLIENT_ID` and `GITHUB_CLIENT_SECRET`.

### Admin Access

Admin role is determined by the backend via the `ADMIN_GITHUB_IDS` environment variable. To grant admin to a GitHub account, find its numeric user ID:

```bash
curl https://api.github.com/users/<github-username> | grep '"id"'
# → "id": 12345678
```

Add the ID to the backend's `.env`:

```env
ADMIN_GITHUB_IDS=12345678
```

Changes take effect the next time that user signs in — no server restart needed.

---

## Configuration

Two environment variables control runtime behavior. Defaults work out of the box for local development. Variables can be set via a `.env` file in the working directory or exported in the shell.

| Variable                 | Default                 | Description               |
| ------------------------ | ----------------------- | ------------------------- |
| `INSIGHTA_API_URL`       | `http://localhost:8000` | Backend API base URL      |
| `INSIGHTA_CALLBACK_PORT` | `8182`                  | Local OAuth callback port |

> The CLI loads `.env` automatically via [dotenvy](https://crates.io/crates/dotenvy) at startup.

---

## Auth Commands

```bash
insighta login     # GitHub OAuth with PKCE — opens browser
insighta logout    # Invalidates session server-side, deletes local credentials
insighta whoami    # Validates session against backend, prints @username (role)
```

Credentials are stored at `~/.insighta/credentials.json`:

```json
{
  "access_token": "eyJ...",
  "refresh_token": "a3f2...",
  "username": "octocat"
}
```

---

## Profile Commands

### List

```bash
insighta profiles list
insighta profiles list --gender male
insighta profiles list --country NG --age-group adult
insighta profiles list --min-age 25 --max-age 40
insighta profiles list --sort-by age --order desc
insighta profiles list --page 2 --limit 20
```

| Flag          | Type                                      | Description                             |
| ------------- | ----------------------------------------- | --------------------------------------- |
| `--gender`    | `male`\|`female`                          | Filter by gender                        |
| `--country`   | ISO α-2                                   | Filter by country code (e.g. `NG`)      |
| `--age-group` | `child`\|`teenager`\|`adult`\|`senior`    | Filter by age group                     |
| `--min-age`   | integer                                   | Minimum age (inclusive)                 |
| `--max-age`   | integer                                   | Maximum age (inclusive)                 |
| `--sort-by`   | `age`\|`created_at`\|`gender_probability` | Sort field                              |
| `--order`     | `asc`\|`desc`                             | Sort direction                          |
| `--page`      | integer                                   | Page number (default: 1)                |
| `--limit`     | integer                                   | Results per page (default: 10, max: 50) |

### Get, Search, Create, Export

```bash
# Get single profile
insighta profiles get <uuid>

# Natural language search
insighta profiles search "young males from nigeria"
insighta profiles search "adults above 30 in japan" --page 2

# Create (admin only)
insighta profiles create --name "Harriet Tubman"

# Export to CSV (saved to current directory)
insighta profiles export
insighta profiles export --gender male --country NG
insighta profiles export --age-group adult --min-age 25 --sort-by age --order desc
```

> `export` accepts the same flags as `list` minus `--page` and `--limit` — it always exports all matching records.

### Output

Every `list` and `search` command renders a formatted UTF-8 table. A spinner animates while requests are in flight. Results include pagination info:

```
Showing page 1 of 203 (2026 total)
```

---

## Authentication Flow

`insighta login` runs a full PKCE flow. No credentials are entered — the browser handles authentication.

**Verifier generation**

All parameters are generated locally before the browser opens:

```
code_verifier  →  32 random bytes, hex-encoded (64 chars)
code_challenge →  BASE64URL-NOPAD( SHA-256(code_verifier) )
state          →  16 random bytes, hex-encoded (CSRF token)
```

**Browser redirect**

The CLI starts a local TCP server on the callback port, then opens:

```
<API_URL>/auth/github
  ?state=<state>
  &code_challenge=<challenge>
  &redirect_uri=http://127.0.0.1:<port>/callback
```

**Callback capture**

GitHub redirects to the local server after authentication. The CLI reads `code` and `state` from the redirect URL and validates `returned_state == state`.

**Token exchange**

```
GET <API_URL>/auth/github/callback
  ?code=<code>
  &state=<state>
  &code_verifier=<verifier>
```

The backend verifies the PKCE challenge, exchanges the code with GitHub, and returns `{ access_token, refresh_token }`. The CLI decodes the username from the JWT payload and saves to `~/.insighta/credentials.json`.

> Authorization window: 3 minutes. If no callback is received, the flow times out — re-run `insighta login`.

---

## Token Handling

Every request attaches `Authorization: Bearer <access_token>` and `X-API-Version: 1`.

**Access token**

| Property | Value                                          |
| -------- | ---------------------------------------------- |
| Format   | JWT, signed HS256                              |
| Expiry   | 3 minutes                                      |
| Claims   | `sub` (UUID), `role`, `username`, `iat`, `exp` |

**Refresh token**

| Property    | Value                              |
| ----------- | ---------------------------------- |
| Format      | 64-char opaque hex string          |
| Expiry      | 5 minutes                          |
| Consumption | One-time use — invalidated on read |

**Refresh lifecycle**

On a `401` response the CLI automatically calls `POST /auth/refresh`, saves the new token pair to `~/.insighta/credentials.json`, and retries the original request — transparent to the user. If the refresh token is also expired, credentials are deleted and the user sees:

```
Session expired. Run 'insighta login' to re-authenticate.
```

---

## Role Enforcement

The backend enforces roles. The CLI passes the Bearer token and surfaces whatever the API returns.

| Role      | Available commands                                           |
| --------- | ------------------------------------------------------------ |
| `admin`   | All commands including `create` and implicit `delete` access |
| `analyst` | `list`, `get`, `search`, `export` only                       |

Attempting a restricted command as an analyst returns:

```
error: API error: Admin access required
```

---

## Natural Language Search

<details>
<summary>How query parsing works</summary>

The backend `/api/profiles/search?q=` endpoint accepts plain-English queries. The CLI sends the query string as-is; all parsing happens server-side.

**Gender keywords**

| Input tokens                                                             | Resolved filter |
| ------------------------------------------------------------------------ | --------------- |
| `male`, `males`, `man`, `men`, `boy`, `boys`                             | `gender=male`   |
| `female`, `females`, `woman`, `women`, `girl`, `girls`, `lady`, `ladies` | `gender=female` |

**Age group keywords**

| Input tokens                                            | Resolved filter            |
| ------------------------------------------------------- | -------------------------- |
| `child`, `children`, `kid`, `kids`                      | `age_group=child`          |
| `teenager`, `teen`, `teens`                             | `age_group=teenager`       |
| `adult`, `adults`, `grownup`, `grownups`, `middle-aged` | `age_group=adult`          |
| `senior`, `seniors`, `old`, `elderly`                   | `age_group=senior`         |
| `young`                                                 | `min_age=16`, `max_age=24` |

**Age range bigrams**

| Input pattern                     | Resolved filter |
| --------------------------------- | --------------- |
| `above N`, `over N`, `at least N` | `min_age=N`     |
| `below N`, `under N`, `at most N` | `max_age=N`     |

**Country**

Matched via `from [country]` or `in [country]`. Supports multi-word country names up to 7 tokens.

**Sort bigrams**

| Input pattern                    | Resolved filter                            |
| -------------------------------- | ------------------------------------------ |
| `top N`, `first N`, `latest N`   | `sort=created_at`, `order=desc`, `limit=N` |
| `last N`, `oldest N`, `bottom N` | `sort=created_at`, `order=asc`, `limit=N`  |

**Example queries**

| Query                      | Filters applied                                         |
| -------------------------- | ------------------------------------------------------- |
| `young males from nigeria` | `gender=male`, `min_age=16`, `max_age=24`, `country=NG` |
| `females above 30`         | `gender=female`, `min_age=30`                           |
| `adults in japan`          | `age_group=adult`, `country=JP`                         |
| `top 5 women`              | `gender=female`, `sort=created_at desc`, `limit=5`      |
| `nigeria`                  | `country=NG`                                            |

</details>

---

## Error Messages

| Situation       | Message                                                     |
| --------------- | ----------------------------------------------------------- |
| Not logged in   | `Not logged in. Run 'insighta login' first.`                |
| Session expired | `Session expired. Run 'insighta login' to re-authenticate.` |
| API error       | `API error: <backend message>`                              |
| Network failure | `HTTP error: <reqwest message>`                             |
| IO failure      | `IO error: <os message>`                                    |

---

## Project Structure

```
src/
├── main.rs          # Entry point, routes commands
├── cli.rs           # Clap command definitions
├── auth.rs          # login / logout / whoami + PKCE logic
├── client.rs        # Authenticated HTTP client with auto-refresh
├── credentials.rs   # Read/write ~/.insighta/credentials.json
├── profiles.rs      # All profile subcommand handlers
├── output.rs        # Spinner + table renderer
├── config.rs        # INSIGHTA_API_URL, INSIGHTA_CALLBACK_PORT
└── error.rs         # CliError enum
```
