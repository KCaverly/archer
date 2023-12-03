use std::time::Instant;

use color_eyre::eyre::Result;
use ratatui::{prelude::*, widgets::*};
use tokio::sync::mpsc::UnboundedSender;

use super::Component;
use crate::config::{Config, KeyBindings};
use crate::styles::{ACTIVE_COLOR, FOCUSED_COLOR, UNFOCUSED_COLOR};
use crate::{action::Action, tui::Frame};

#[derive(Default)]
pub struct MessageInput {
    command_tx: Option<UnboundedSender<Action>>,
    config: Config,
    active: bool,
    focused: bool,
}

impl MessageInput {
    pub fn new(focused: bool) -> Self {
        Self {
            focused,
            ..Default::default()
        }
    }
}

impl Component for MessageInput {
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
            Action::FocusViewer => {
                self.focused = false;
                self.active = false;
            }
            Action::FocusInput => {
                self.focused = true;
                self.active = false;
            }
            Action::ActivateInput => self.active = true,
            Action::DeactivateInput => self.active = false,

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
                    .title("Input")
                    .title_alignment(Alignment::Left)
                    .borders(Borders::ALL)
                    .border_type(BorderType::Thick)
                    .style(
                        Style::default()
                            .fg(if self.active {
                                ACTIVE_COLOR
                            } else if self.focused {
                                FOCUSED_COLOR
                            } else {
                                UNFOCUSED_COLOR
                            })
                            .bg(Color::Black),
                    ),
            )
            .alignment(Alignment::Left)
            .wrap(Wrap { trim: true });
        f.render_widget(paragraph, layout[1]);
        Ok(())
    }
}
