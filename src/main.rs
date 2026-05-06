use feedparser_rs::{ParsedFeed, parse_url};
use ratatui::{
    DefaultTerminal, Frame,
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::List,
    widgets::ListItem,
};

fn main() -> color_eyre::Result<()> {
    color_eyre::install()?;
    let feed = parse_url("https://feeds.stevedylan.dev/feed.xml", None, None, None)?;
    ratatui::run(|t| app(t, &feed))?;
    Ok(())
}

fn app(terminal: &mut DefaultTerminal, feed: &ParsedFeed) -> std::io::Result<()> {
    loop {
        terminal.draw(|f| render(f, feed))?;
        if crossterm::event::read()?.is_key_press() {
            break Ok(());
        }
    }
}

fn render(frame: &mut Frame, feed: &ParsedFeed) {
    let dim = Style::new().fg(Color::DarkGray);
    let author_style = Style::new()
        .fg(Color::DarkGray)
        .add_modifier(Modifier::ITALIC);

    let items: Vec<ListItem> = feed
        .entries
        .iter()
        .map(|e| {
            let date = e
                .published
                .as_ref()
                .map(|d| d.to_string())
                .unwrap_or_else(|| "-".into());
            let title = e.title.as_deref().unwrap_or("(untitled)");
            let author = e
                .authors
                .first()
                .and_then(|a| a.name.as_deref())
                .unwrap_or("anon");
            ListItem::new(Text::from(vec![
                Line::from(Span::styled(date, dim)),
                Line::from(title.to_string()),
                Line::from(Span::styled(author.to_string(), author_style)),
                Line::from(""),
            ]))
        })
        .collect();
    frame.render_widget(List::new(items), frame.area());
}
