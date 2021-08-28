use std::{collections::VecDeque, io};

use tui::{
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Corner, Direction, Layout},
    widgets::{Block, Borders, List, ListItem, Paragraph},
    Terminal,
};

#[derive(Debug)]
pub enum Command {
    Draw,
    SendOutput(String),
    SetInput(String),
    SetRats(Vec<String>),
    Quit,
}

pub struct Gui {
    terminal: Terminal<CrosstermBackend<io::Stdout>>,
}

struct State {
    output: VecDeque<String>,
    input: String,
    rats: Vec<String>,
}

impl Gui {
    pub fn run(
        mut gui_rx: tokio::sync::mpsc::Receiver<Command>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        crossterm::terminal::enable_raw_mode()?;
        let stdout = io::stdout();
        let backend = CrosstermBackend::new(stdout);
        let terminal = Terminal::new(backend)?;
        crossterm::execute!(io::stdout(), crossterm::terminal::EnterAlternateScreen)?;

        let mut gui = Gui { terminal };

        let mut state = State {
            output: VecDeque::new(),
            input: "".to_string(),
            rats: Vec::new(),
        };

        while let Some(msg) = gui_rx.blocking_recv() {
            match msg {
                Command::Draw => {
                    gui.draw(&state)?;
                }

                Command::SendOutput(output) => {
                    state.output.push_back(output);

                    if state.output.len() > 100 {
                        state.output.pop_front();
                    }
                }

                Command::SetInput(input) => {
                    state.input = input;
                }

                Command::SetRats(rats) => {
                    state.rats = rats;
                }

                Command::Quit => {
                    break;
                }
            }
        }

        Ok(())
    }

    fn draw(&mut self, state: &State) -> Result<(), Box<dyn std::error::Error>> {
        self.terminal.draw(|f| {
            let horizontal_chunks = Layout::default()
                .direction(Direction::Horizontal)
                .margin(1)
                .constraints([Constraint::Percentage(80), Constraint::Percentage(20)].as_ref())
                .split(f.size());

            let main_chunks = Layout::default()
                .direction(Direction::Vertical)
                .margin(0)
                .constraints([Constraint::Min(1), Constraint::Length(3)].as_ref())
                .split(horizontal_chunks[0]);

            let output_area = main_chunks[0];
            let input_area = main_chunks[1];
            let list_area = horizontal_chunks[1];

            let output_block = Block::default().borders(Borders::ALL);

            let input_block = Block::default().borders(Borders::ALL);

            let list_block = Block::default().title("Rats").borders(Borders::ALL);

            f.render_widget(list_block, list_area);

            // This will fail when provided something more fancy than ascii and I am okay with that for now
            let mut display_slice: &str = &state.input;
            while display_slice.len() > (input_area.width as usize - 3) {
                let diff = display_slice.len() - (input_area.width as usize - 3);
                display_slice = &display_slice[diff..display_slice.len()];
            }

            let input = Paragraph::new(display_slice)
                .alignment(Alignment::Left)
                .block(input_block);

            let mut listitems = Vec::with_capacity(output_area.height as usize);

            for entry in state.output.iter().rev().take(output_area.height as usize) {
                let entry_lines = entry
                    .trim_end()
                    .as_bytes()
                    .chunks(output_area.width as usize - 2)
                    .rev()
                    .map(|bytearray| std::str::from_utf8(bytearray).unwrap());

                for line in entry_lines {
                    listitems.push(ListItem::new(line));
                }
            }

            let message_list = List::new(listitems)
                .block(output_block)
                .start_corner(Corner::BottomLeft);

            f.render_widget(input, input_area);
            f.render_widget(message_list, output_area);

            f.set_cursor(
                input_area.x + display_slice.len() as u16 + 1,
                input_area.y + 1,
            );
        })?;

        Ok(())
    }
}

impl Drop for Gui {
    fn drop(&mut self) {
        crossterm::execute!(io::stdout(), crossterm::terminal::LeaveAlternateScreen).unwrap();
        crossterm::terminal::disable_raw_mode().unwrap();
    }
}
