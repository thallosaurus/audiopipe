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

use crate::{CpalStats, UdpStats};

pub fn run_tui(rx: Receiver<UdpStats>, cpal_rx: Receiver<CpalStats>) -> io::Result<()> {
    let mut terminal = ratatui::init();
    let app_result = App::default().set_receiver(rx, cpal_rx).run(&mut terminal);
    ratatui::restore();
    app_result
}

#[derive(Debug, Default)]
pub struct App {
    exit: bool,
    udp_rx: Arc<Mutex<Option<Receiver<UdpStats>>>>,
    cpal_rx: Arc<Mutex<Option<Receiver<CpalStats>>>>,
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

    pub fn set_receiver(
        &mut self,
        udp_rx: Receiver<UdpStats>,
        cpal_rx: Receiver<CpalStats>,
    ) -> &mut Self {
        self.udp_rx = Arc::new(Mutex::new(Some(udp_rx)));
        self.cpal_rx = Arc::new(Mutex::new(Some(cpal_rx)));
        self
    }

    fn draw(&self, frame: &mut Frame) {
        frame.render_widget(self, frame.area());
    }

    fn handle_events(&mut self) -> io::Result<()> {
        if let Ok(avail) = event::poll(Duration::from_millis(500)) {
            if avail {
                match event::read()? {
                    // it's important to check that the event is a key press event as
                    // crossterm also emits key release and repeat events on Windows.
                    Event::Key(key_event) if key_event.kind == KeyEventKind::Press => {
                        self.handle_key_event(key_event)
                    }
                    _ => {}
                };
            }
        }
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

        let udp_rx = self.udp_rx.lock().unwrap();
        let cpal_rx = self.cpal_rx.lock().unwrap();

        let udp_stats = udp_rx
            .as_ref()
            .expect("no receiver set")
            .recv_timeout(Duration::from_millis(500));

        let cpal_stats = cpal_rx
            .as_ref()
            .expect("no receiver set")
            .recv_timeout(Duration::from_millis(500));

        let counter_text;
        if let Ok(stats) = udp_stats {
            counter_text = format!("Occupied Size: {}, Received Samples: {}", stats.occupied_buffer, stats.received);

        } else {
            /*counter_text = Text::from(vec![Line::from(vec![
                //"Hello World!".into()
                "No Data Available".into(),
            ])]);*/
            counter_text = format!("No Data Available");
        }

        let cpal_stats_text;
        if let Ok(stats) = cpal_stats {
            cpal_stats_text = format!("Requested CPAL Buffer Length: {}", stats.requested_sample_length)
        } else {
            cpal_stats_text = format!("No recent audio request")
        }

        let counter_text = Text::from(vec![Line::from(vec![
                //"Hello World!".into()
                counter_text.into(),
                cpal_stats_text.into()
            ])]);

        Paragraph::new(counter_text)
            .centered()
            .block(block)
            .render(area, buf);
    }
}
