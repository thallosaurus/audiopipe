use std::{
    io,
    sync::{Arc, Mutex, mpsc::Receiver},
    time::Duration,
};

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind};
use ratatui::{
    DefaultTerminal, Frame,
    buffer::Buffer,
    layout::Rect,
    style::Stylize,
    symbols::border,
    text::{Line, Text},
    widgets::{Block, Paragraph, Widget},
};

use crate::Stats;

pub fn run_tui(rx: Receiver<Stats>) -> io::Result<()> {
    let mut terminal = ratatui::init();
    let app_result = App::default().set_receiver(rx).run(&mut terminal);
    ratatui::restore();
    app_result
}

#[derive(Debug, Default)]
pub struct App {
    exit: bool,
    receiver: Arc<Mutex<Option<Receiver<Stats>>>>,
}

impl App {
    /// runs the application's main loop until the user quits
    pub fn run(&mut self, terminal: &mut DefaultTerminal) -> io::Result<()> {
        while !self.exit {
            terminal.draw(|frame| self.draw(frame))?;
            self.handle_events()?;
        }
        Ok(())
    }

    pub fn set_receiver(&mut self, receiver: Receiver<Stats>) -> &mut Self {
        self.receiver = Arc::new(Mutex::new(Some(receiver)));
        self
    }

    fn draw(&self, frame: &mut Frame) {
        frame.render_widget(self, frame.area());
    }

    fn handle_events(&mut self) -> io::Result<()> {
        match event::read()? {
            // it's important to check that the event is a key press event as
            // crossterm also emits key release and repeat events on Windows.
            Event::Key(key_event) if key_event.kind == KeyEventKind::Press => {
                self.handle_key_event(key_event)
            }
            _ => {}
        };
        Ok(())
    }

    fn handle_key_event(&mut self, key_event: KeyEvent) {
        match key_event.code {
            KeyCode::Char('q') => self.exit(),
            //KeyCode::Left => self.decrement_counter(),
            //KeyCode::Right => self.increment_counter(),
            _ => {}
        }
    }

    fn exit(&mut self) {
        self.exit = true;
    }
}

impl Widget for &App {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let title = Line::from(" Counter App Tutorial ".bold());
        let instructions = Line::from(vec![
            " Decrement ".into(),
            "<Left>".blue().bold(),
            " Increment ".into(),
            "<Right>".blue().bold(),
            " Quit ".into(),
            "<Q> ".blue().bold(),
        ]);
        let block = Block::bordered()
            .title(title.centered())
            .title_bottom(instructions.centered())
            .border_set(border::THICK);

        let r = self.receiver.lock().unwrap();

        let stats = r
            .as_ref()
            .expect("no receiver set")
            .recv_timeout(Duration::from_millis(500));

        let mut counter_text;
        if let Ok(stats) = stats {
            counter_text = Text::from(vec![Line::from(vec![
                //"Hello World!".into()
                format!("Occupied Size: {}", stats.occupied_buffer).into(),
                format!("Received Samples: {}", stats.received).into(),
            ])]);
        } else {
            counter_text = Text::from(vec![Line::from(vec![
                //"Hello World!".into()
                "No Data Available".into(),
            ])]);
        }

        Paragraph::new(counter_text)
            .centered()
            .block(block)
            .render(area, buf);
    }
}
