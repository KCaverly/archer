use ratatui::{
    layout::Alignment,
    prelude::{Constraint, Direction, Layout},
    style::{Color, Style},
    widgets::{Block, BorderType, Borders, Paragraph, Wrap},
    Frame,
};

use crate::app::{App, CurrentFocus};

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

    let active_color = Color::Cyan;
    let inactive_color = Color::LightBlue;
    let insert_color = Color::Yellow;

    frame.render_widget(
        Paragraph::new(if let Some(message) = app.messages.get(0) {
            message.clone()
        } else {
            "".to_string()
        })
        .block(
            Block::default()
                .title("Bond")
                .title_alignment(Alignment::Left)
                .borders(Borders::ALL)
                .border_type(BorderType::Thick),
        )
        .style(
            Style::default()
                .fg(match app.current_focus {
                    CurrentFocus::Viewer => active_color,
                    _ => inactive_color,
                })
                .bg(Color::Black),
        )
        .alignment(Alignment::Left)
        .wrap(Wrap { trim: true }),
        layout[0],
    );
    frame.render_widget(
        Paragraph::new(app.user_input.as_str())
            .block(
                Block::default()
                    .title("Bond")
                    .title_alignment(Alignment::Left)
                    .borders(Borders::ALL)
                    .border_type(BorderType::Thick),
            )
            .style(
                Style::default()
                    .fg(match app.current_focus {
                        CurrentFocus::Input { insert } => {
                            if insert {
                                insert_color
                            } else {
                                active_color
                            }
                        }
                        _ => inactive_color,
                    })
                    .bg(Color::Black),
            )
            .alignment(Alignment::Left)
            .wrap(Wrap { trim: true }),
        layout[1],
    );
}
