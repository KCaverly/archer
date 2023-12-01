use ratatui::{
    layout::Alignment,
    prelude::{Constraint, Direction, Layout},
    style::{Color, Style},
    text::{Line, Span, Text},
    widgets::{Block, BorderType, Borders, Paragraph, Wrap},
    Frame,
};

use crate::app::{App, CurrentFocus, MessageRole};

const ACTIVE_COLOR: Color = Color::LightBlue;
const INACTIVE_COLOR: Color = Color::Gray;
const INPUT_COLOR: Color = Color::Yellow;

pub fn render_user_input(app: &mut App) -> Paragraph {
    Paragraph::new(app.user_input.as_str())
        .block(
            Block::default()
                .title("Input")
                .title_alignment(Alignment::Left)
                .borders(Borders::ALL)
                .border_type(BorderType::Thick),
        )
        .style(
            Style::default()
                .fg(match app.current_focus {
                    CurrentFocus::Input { insert } => {
                        if insert {
                            INPUT_COLOR
                        } else {
                            ACTIVE_COLOR
                        }
                    }
                    _ => INACTIVE_COLOR,
                })
                .bg(Color::Black),
        )
        .alignment(Alignment::Left)
        .wrap(Wrap { trim: true })
}

pub fn render_messages(app: &mut App) -> Paragraph {
    let mut lines = Vec::new();
    for message in &app.messages {
        match message.role {
            MessageRole::User => {
                lines.push(Line::from(vec![Span::styled(
                    "User",
                    Style::default().fg(Color::Red),
                )]));
            }
            MessageRole::Assistant => {
                lines.push(Line::from(vec![Span::styled(
                    "Assistant",
                    Style::default().fg(Color::Blue),
                )]));
            }
        }

        let content = message.content.clone();
        lines.push(Line::from(vec![Span::styled(
            format!("{content}"),
            Style::default().fg(Color::White),
        )]))
    }

    let text = Text::from(lines);
    Paragraph::new(text)
        .block(
            Block::default()
                .title("Viewer")
                .title_alignment(Alignment::Left)
                .borders(Borders::ALL)
                .border_type(BorderType::Thick)
                .style(
                    Style::default()
                        .fg(match app.current_focus {
                            CurrentFocus::Viewer => ACTIVE_COLOR,
                            _ => INACTIVE_COLOR,
                        })
                        .bg(Color::Black),
                ),
        )
        .alignment(Alignment::Left)
        .wrap(Wrap { trim: true })
}

/// Renders the user interface widgets.
pub fn render(app: &mut App, frame: &mut Frame) {
    // This is where you add new widgets.
    // See the following resources:
    // - https://docs.rs/ratatui/latest/ratatui/widgets/index.html
    // - https://github.com/ratatui-org/ratatui/tree/master/examples

    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints(vec![Constraint::Percentage(90), Constraint::Percentage(10)])
        .split(frame.size());

    frame.render_widget(render_messages(app), layout[0]);
    frame.render_widget(render_user_input(app), layout[1]);
}
