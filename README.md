# bullets

Minimal terminal RSS reader. Pass one or more feed URLs, browse entries, open in browser.

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

## Install

```sh
cargo install --path .
```

## License

MIT — Copyright (c) Steve <contact@stevedylan.dev>
