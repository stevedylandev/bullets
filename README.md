<div align="center">

# bullets

![icon](https://files.stevedylan.dev/bullets-icon-2.png)

Minimal terminal RSS/Atom feed browser. Pass one or more feed URLs, browse entries, open in browser.

</div>

## Philosophy

![demo](https://files.stevedylan.dev/bullets-demo.png)

Designed to be the simplest RSS/Atom feed browser

- No read/unread
- No cron, db, or notifications
- No in-app reader

## Install

**Homebrew**

```
brew install stevedylandev/tap/bullets
```

**Cargo**

```
cargo install bullets
```

[**Other Install Methods**](https://github.com/stevedylandev/bullets/releases)

## Usage

```sh
bullets <feed-url> [feed-url ...]
```

Or set feeds via environment variable (comma-separated):

```sh
export BULLETS_FEEDS=https://example.com/feed.xml,stevedylan.dev
```

Feed discovery included — bare domains (e.g. `stevedylan.dev`) are probed for RSS/Atom feeds automatically.

## Keybindings

| Key | Action |
|-----|--------|
| `j` / `↓` | Next entry |
| `k` / `↑` | Prev entry |
| `Enter` | Open in browser |
| `q` | Quit |

## License

[MIT](LICENSE)
