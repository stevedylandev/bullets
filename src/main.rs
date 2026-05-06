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

fn main() -> color_eyre::Result<()> {
    color_eyre::install()?;
    let urls: Vec<String> = std::env::args().skip(1).collect();
    if urls.is_empty() {
        return Err(color_eyre::eyre::eyre!("Usage: bullets <feed-url> [feed-url ...]"));
    }
    let feeds: Vec<ParsedFeed> = urls
        .iter()
        .map(|url| parse_url(url, None, None, None))
        .collect::<Result<_, _>>()?;

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
                    let next = state.selected().map(|i| (i + 1).min(len - 1)).unwrap_or(0);
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
    let day = dt.format("%e").to_string().trim().parse::<u32>().unwrap_or(0);
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
