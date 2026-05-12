use chrono::{DateTime, Datelike, Utc};
use color_eyre::eyre::WrapErr;
use crossterm::event::{KeyCode, KeyEvent};
use feedparser_rs::{ParsedFeed, parse_url};
use ratatui::{
    DefaultTerminal, Frame,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Padding, Paragraph},
};

struct Item {
    title: String,
    author: String,
    date: String,
    url: Option<String>,
    published: Option<DateTime<Utc>>,
}

fn normalize_url(s: &str) -> String {
    if s.starts_with("http://") || s.starts_with("https://") {
        s.to_string()
    } else {
        format!("https://{s}")
    }
}

fn is_bare_domain(url: &str) -> bool {
    let rest = url.split_once("://").map(|(_, r)| r).unwrap_or(url);
    rest.split_once('/')
        .map_or(true, |(_, p)| p.trim_matches('/').is_empty())
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

fn build_agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_global(Some(std::time::Duration::from_secs(10)))
        .build()
        .into()
}

fn discover_feed(agent: &ureq::Agent, input: &str) -> color_eyre::Result<String> {
    let url = normalize_url(input);
    if !is_bare_domain(&url) {
        return Ok(url);
    }
    let html = agent
        .get(&url)
        .call()
        .wrap_err_with(|| format!("fetch {url}"))?
        .body_mut()
        .read_to_string()
        .wrap_err_with(|| format!("read {url}"))?;
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
        if agent
            .get(&candidate)
            .call()
            .map(|r| r.status() == 200)
            .unwrap_or(false)
        {
            return Ok(candidate);
        }
    }
    Err(color_eyre::eyre::eyre!("No feed found for: {input}"))
}

fn load_feeds(urls: &[String]) -> Vec<ParsedFeed> {
    let agent = build_agent();
    urls.iter()
        .filter_map(|url| {
            let resolved = discover_feed(&agent, url)
                .map_err(|e| eprintln!("warning: skipping {url}: {e}"))
                .ok()?;
            parse_url(&resolved, None, None, None)
                .map_err(|e| eprintln!("warning: failed to parse feed {url}: {e}"))
                .ok()
        })
        .collect()
}

fn collect_items(feeds: Vec<ParsedFeed>) -> Vec<Item> {
    let mut items: Vec<Item> = feeds
        .into_iter()
        .flat_map(|f| {
            let feed_title = f.feed.title.clone();
            f.entries.into_iter().map(move |e| {
                let author = e
                    .authors
                    .first()
                    .and_then(|a| a.name.as_ref().map(|n| n.to_string()))
                    .or_else(|| feed_title.as_ref().map(|t| t.to_string()))
                    .unwrap_or_else(|| "anon".into());
                let date = e.published.map(fmt_date).unwrap_or_else(|| "-".into());
                Item {
                    title: e
                        .title
                        .map(|t| t.to_string())
                        .unwrap_or_else(|| "(untitled)".into()),
                    author,
                    date,
                    url: e.links.into_iter().next().map(|l| l.href.to_string()),
                    published: e.published,
                }
            })
        })
        .collect();
    items.sort_by(|a, b| b.published.cmp(&a.published));
    items
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

    let feeds = load_feeds(&urls);
    if feeds.is_empty() {
        eprintln!("No feeds loaded successfully.");
        std::process::exit(1);
    }

    let items = collect_items(feeds);
    if items.is_empty() {
        eprintln!("No entries found in any feed.");
        std::process::exit(1);
    }

    ratatui::run(|t| app(t, &items))?;
    Ok(())
}

fn app(terminal: &mut DefaultTerminal, items: &[Item]) -> std::io::Result<()> {
    let mut selected = 0;
    let mut scroll_offset = 0;

    loop {
        terminal.draw(|f| render(f, items, selected, &mut scroll_offset))?;

        if let crossterm::event::Event::Key(KeyEvent { code, .. }) = crossterm::event::read()? {
            let len = items.len();
            match code {
                KeyCode::Char('q') => break,
                KeyCode::Char('j') | KeyCode::Down => {
                    selected = (selected + 1).min(len.saturating_sub(1));
                }
                KeyCode::Char('k') | KeyCode::Up => {
                    selected = selected.saturating_sub(1);
                }
                KeyCode::Enter => {
                    if let Some(url) = items[selected].url.as_deref() {
                        let _ = open::that(url);
                    }
                }
                _ => {}
            }
        }
    }

    Ok(())
}

fn fmt_date(dt: DateTime<Utc>) -> String {
    let day = dt.day();
    let suffix = match day {
        1 | 21 | 31 => "st",
        2 | 22 => "nd",
        3 | 23 => "rd",
        _ => "th",
    };
    format!("{} {}{}, {}", dt.format("%B"), day, suffix, dt.format("%Y"))
}

fn render(frame: &mut Frame, items: &[Item], selected: usize, scroll_offset: &mut u16) {
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

    let mut selected_start = 0;
    let mut selected_end = 0;
    let mut lines = Vec::new();
    for (i, item) in items.iter().enumerate() {
        if i == selected {
            selected_start = lines.len();
        }

        let bar = if i == selected { "▌ " } else { "  " };
        lines.push(Line::from(vec![
            Span::raw(bar),
            Span::styled(item.date.clone(), dim),
        ]));
        for wrapped in textwrap::wrap(&item.title, title_width.max(1)) {
            lines.push(Line::from(vec![
                Span::raw(bar),
                Span::raw(wrapped.into_owned()),
            ]));
        }
        lines.push(Line::from(vec![
            Span::raw(bar),
            Span::styled(item.author.clone(), author_style),
        ]));
        lines.push(Line::from(""));

        if i == selected {
            selected_end = lines.len();
        }
    }

    let viewport_height = inner.height as usize;
    if viewport_height > 0 {
        let mut offset = *scroll_offset as usize;
        if selected_start < offset {
            offset = selected_start;
        } else if selected_end > offset + viewport_height {
            offset = selected_end.saturating_sub(viewport_height);
        }
        offset = offset.min(lines.len().saturating_sub(viewport_height));
        *scroll_offset = offset.min(u16::MAX as usize) as u16;
    }

    frame.render_widget(block, center);
    frame.render_widget(
        Paragraph::new(Text::from(lines)).scroll((*scroll_offset, 0)),
        inner,
    );
}
