use chrono::NaiveDateTime;
use crossterm::event::{KeyCode, KeyEvent};
use feedparser_rs::{Entry, ParsedFeed, parse_url};
use ratatui::{
    DefaultTerminal, Frame,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, List, ListItem, ListState, Padding},
};

fn normalize_url(s: &str) -> String {
    if s.starts_with("http://") || s.starts_with("https://") {
        s.to_string()
    } else {
        format!("https://{s}")
    }
}

fn is_bare_domain(url: &str) -> bool {
    let rest = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))
        .unwrap_or(url);
    let path = rest.find('/').map(|i| &rest[i..]).unwrap_or("");
    path.trim_matches('/').is_empty()
}

fn find_feed_link(html: &str, base_url: &str) -> Option<String> {
    let base = base_url.trim_end_matches('/');
    let lower = html.to_lowercase();
    let mut pos = 0;
    while let Some(tag_start) = lower[pos..].find("<link") {
        let abs = pos + tag_start;
        let tag_end = lower[abs..].find('>')? + abs;
        let tag = &html[abs..=tag_end];
        let tag_lower = tag.to_lowercase();
        let is_feed =
            tag_lower.contains("application/rss+xml") || tag_lower.contains("application/atom+xml");
        if is_feed {
            if let Some(href) = extract_attr(tag, "href") {
                let resolved = if href.starts_with("http://") || href.starts_with("https://") {
                    href
                } else if href.starts_with('/') {
                    format!("{base}{href}")
                } else {
                    format!("{base}/{href}")
                };
                return Some(resolved);
            }
        }
        pos = tag_end + 1;
    }
    None
}

fn extract_attr(tag: &str, attr: &str) -> Option<String> {
    let search = format!("{attr}=");
    let lower = tag.to_lowercase();
    let start = lower.find(&search)? + search.len();
    let rest = &tag[start..];
    let (quote, end_char) = if rest.starts_with('"') {
        (&rest[1..], '"')
    } else if rest.starts_with('\'') {
        (&rest[1..], '\'')
    } else {
        return None;
    };
    let end = quote.find(end_char)?;
    Some(quote[..end].to_string())
}

fn discover_feed(input: &str) -> color_eyre::Result<String> {
    let url = normalize_url(input);
    if !is_bare_domain(&url) {
        return Ok(url);
    }
    let timeout = std::time::Duration::from_secs(10);
    let agent = ureq::Agent::config_builder()
        .timeout_global(Some(timeout))
        .build()
        .into();
    let html = ureq::Agent::new_with_config(agent)
        .get(&url)
        .call()
        .map_err(|e| color_eyre::eyre::eyre!("Failed to fetch {url}: {e}"))?
        .body_mut()
        .read_to_string()
        .map_err(|e| color_eyre::eyre::eyre!("Failed to read response from {url}: {e}"))?;
    if let Some(feed_url) = find_feed_link(&html, &url) {
        return Ok(feed_url);
    }
    let base = url.trim_end_matches('/');
    const PATHS: &[&str] = &[
        "/feed.xml",
        "/rss.xml",
        "/atom.xml",
        "/feed",
        "/rss",
        "/index.xml",
        "/feeds/posts/default",
        "/blog/feed.xml",
        "/blog/rss.xml",
    ];
    for path in PATHS {
        let candidate = format!("{base}{path}");
        if ureq::get(&candidate)
            .call()
            .map(|r: ureq::http::Response<ureq::Body>| r.status() == 200)
            .unwrap_or(false)
        {
            return Ok(candidate);
        }
    }
    Err(color_eyre::eyre::eyre!("No feed found for: {input}"))
}

fn main() -> color_eyre::Result<()> {
    color_eyre::install()?;
    let mut urls: Vec<String> = std::env::args().skip(1).collect();
    if urls.is_empty() {
        if let Ok(val) = std::env::var("BULLETS_FEEDS") {
            urls = val
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
        }
    }
    if urls.is_empty() {
        eprintln!("No feeds provided.\n");
        eprintln!("Usage:");
        eprintln!("  bullets <feed-url> [feed-url ...]");
        eprintln!("  bullets https://example.com/feed.xml other.com/rss");
        eprintln!("\nOr set the BULLETS_FEEDS environment variable:");
        eprintln!("  export BULLETS_FEEDS=https://example.com/feed.xml,https://other.com/rss");
        std::process::exit(1);
    }
    let feeds: Vec<ParsedFeed> = urls
        .iter()
        .filter_map(|url| {
            let resolved = match discover_feed(url) {
                Ok(r) => r,
                Err(e) => {
                    eprintln!("warning: skipping {url}: {e}");
                    return None;
                }
            };
            match parse_url(&resolved, None, None, None) {
                Ok(feed) => Some(feed),
                Err(e) => {
                    eprintln!("warning: failed to parse feed {url}: {e}");
                    None
                }
            }
        })
        .collect();

    if feeds.is_empty() {
        eprintln!("No feeds loaded successfully.");
        std::process::exit(1);
    }

    let mut entries: Vec<(&Entry, Option<&str>)> = feeds
        .iter()
        .flat_map(|f| {
            let title = f.feed.title.as_deref();
            f.entries.iter().map(move |e| (e, title))
        })
        .collect();
    entries.sort_by(|a, b| {
        let da = a.0.published.as_ref().map(|d| d.to_string());
        let db = b.0.published.as_ref().map(|d| d.to_string());
        db.cmp(&da)
    });

    if entries.is_empty() {
        eprintln!("No entries found in any feed.");
        std::process::exit(1);
    }
    ratatui::run(|t| app(t, &entries))?;
    Ok(())
}

fn app(terminal: &mut DefaultTerminal, entries: &[(&Entry, Option<&str>)]) -> std::io::Result<()> {
    let mut state = ListState::default();
    state.select(Some(0));

    loop {
        terminal.draw(|f| render(f, entries, &mut state))?;

        if let crossterm::event::Event::Key(KeyEvent { code, .. }) = crossterm::event::read()? {
            let len = entries.len();
            match code {
                KeyCode::Char('q') => break,
                KeyCode::Char('j') | KeyCode::Down => {
                    let next = state
                        .selected()
                        .map(|i| (i + 1).min(len.saturating_sub(1)))
                        .unwrap_or(0);
                    state.select(Some(next));
                }
                KeyCode::Char('k') | KeyCode::Up => {
                    let prev = state.selected().map(|i| i.saturating_sub(1)).unwrap_or(0);
                    state.select(Some(prev));
                }
                KeyCode::Enter => {
                    if let Some(i) = state.selected() {
                        if let Some(url) = entries[i].0.links.first().map(|l| l.href.as_str()) {
                            let _ = open::that(url);
                        }
                    }
                }
                _ => {}
            }
        }
    }

    Ok(())
}

fn fmt_date(raw: &str) -> String {
    let Ok(dt) = NaiveDateTime::parse_from_str(raw, "%Y-%m-%d %H:%M:%S UTC") else {
        return raw.to_string();
    };
    let day = dt
        .format("%e")
        .to_string()
        .trim()
        .parse::<u32>()
        .unwrap_or(0);
    let suffix = match day {
        1 | 21 | 31 => "st",
        2 | 22 => "nd",
        3 | 23 => "rd",
        _ => "th",
    };
    format!("{} {}{}, {}", dt.format("%B"), day, suffix, dt.format("%Y"))
}

fn wrap_text(text: &str, max_width: usize) -> Vec<String> {
    if max_width == 0 {
        return vec![text.to_string()];
    }
    let mut lines: Vec<String> = Vec::new();
    let mut current = String::new();
    for word in text.split_whitespace() {
        if current.is_empty() {
            current.push_str(word);
        } else if current.len() + 1 + word.len() <= max_width {
            current.push(' ');
            current.push_str(word);
        } else {
            lines.push(current);
            current = word.to_string();
        }
    }
    if !current.is_empty() {
        lines.push(current);
    }
    if lines.is_empty() {
        lines.push(String::new());
    }
    lines
}

fn render(frame: &mut Frame, entries: &[(&Entry, Option<&str>)], state: &mut ListState) {
    let outer = frame.area();
    let [_, center, _] = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Fill(1),
            Constraint::Max(80),
            Constraint::Fill(1),
        ])
        .areas(outer);

    let block = Block::new().padding(Padding::symmetric(2, 1));
    let inner = block.inner(center);
    let title_width = inner.width.saturating_sub(2) as usize;

    let dim = Style::new().fg(Color::DarkGray);
    let author_style = Style::new()
        .fg(Color::DarkGray)
        .add_modifier(Modifier::ITALIC);
    let highlight = Style::new();

    let selected = state.selected();
    let items: Vec<ListItem> = entries
        .iter()
        .enumerate()
        .map(|(i, (e, feed_title))| {
            let bar = if selected == Some(i) { "▌ " } else { "  " };
            let date = e
                .published
                .as_ref()
                .map(|d| fmt_date(&d.to_string()))
                .unwrap_or_else(|| "-".into());
            let title = e.title.as_deref().unwrap_or("(untitled)");
            let author = e
                .authors
                .first()
                .and_then(|a| a.name.as_deref())
                .or(*feed_title)
                .unwrap_or("anon");

            let mut lines = vec![Line::from(vec![Span::raw(bar), Span::styled(date, dim)])];
            for wrapped in wrap_text(title, title_width) {
                lines.push(Line::from(vec![Span::raw(bar), Span::raw(wrapped)]));
            }
            lines.push(Line::from(vec![
                Span::raw(bar),
                Span::styled(author.to_string(), author_style),
            ]));
            lines.push(Line::from(""));

            ListItem::new(Text::from(lines))
        })
        .collect();

    frame.render_widget(block, center);
    frame.render_stateful_widget(
        List::new(items)
            .highlight_style(highlight)
            .highlight_symbol(""),
        inner,
        state,
    );
}
