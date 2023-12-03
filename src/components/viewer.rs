use std::time::Instant;

use color_eyre::eyre::Result;
use ratatui::{prelude::*, widgets::*};

use super::Component;
use crate::styles::{ACTIVE_COLOR, FOCUSED_COLOR, UNFOCUSED_COLOR};
use crate::{action::Action, tui::Frame};
use tokio::sync::mpsc::UnboundedSender;

use crate::config::{Config, KeyBindings};

#[derive(Default)]
pub struct Viewer {
    command_tx: Option<UnboundedSender<Action>>,
    config: Config,
    focused: bool,
}

impl Viewer {
    pub fn new(focused: bool) -> Self {
        Self {
            focused,
            ..Default::default()
        }
    }
}

impl Component for Viewer {
    fn register_action_handler(&mut self, tx: UnboundedSender<Action>) -> Result<()> {
        self.command_tx = Some(tx);
        Ok(())
    }

    fn register_config_handler(&mut self, config: Config) -> Result<()> {
        self.config = config;
        Ok(())
    }

    fn update(&mut self, action: Action) -> Result<Option<Action>> {
        match action {
            Action::FocusInput => {
                self.focused = false;
            }
            Action::FocusViewer => {
                self.focused = true;
            }
            _ => {}
        }
        Ok(None)
    }

    fn draw(&mut self, f: &mut Frame<'_>, rect: Rect) -> Result<()> {
        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints(vec![Constraint::Percentage(90), Constraint::Percentage(10)])
            .split(rect);

        let paragraph = Paragraph::new("")
            .block(
                Block::default()
                    .title("Viewer")
                    .title_alignment(Alignment::Left)
                    .borders(Borders::ALL)
                    .border_type(BorderType::Thick)
                    .style(
                        Style::default()
                            .fg(if self.focused {
                                FOCUSED_COLOR
                            } else {
                                UNFOCUSED_COLOR
                            })
                            .bg(Color::Black),
                    ),
            )
            .alignment(Alignment::Left)
            .wrap(Wrap { trim: true });
        f.render_widget(paragraph, layout[0]);
        Ok(())
    }
}
