use std::{
    io,
    sync::{Arc, Mutex, mpsc::Receiver},
    time::Duration,
};

use cpal::{Device, traits::DeviceTrait};
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind};
use ratatui::{
    DefaultTerminal, Frame,
    buffer::Buffer,
    layout::Rect,
    style::Stylize,
    symbols::border,
    text::Line,
    widgets::{Block, Paragraph, Widget},
};

use super::{CpalStats, UdpStats};

pub fn run_tui(
    device: &Device,
    rx: Receiver<UdpStats>,
    cpal_rx: Receiver<CpalStats>,
) -> io::Result<()> {
    let mut terminal = ratatui::init();

    let mut app = App::default();
    app.set_receiver(rx, cpal_rx);
    app.device_name = device.name().unwrap_or("Unknown Device Name".into());

    let app_result = app.run(&mut terminal);
    ratatui::restore();
    app_result
}

#[derive(Debug, Default)]
pub struct App {
    exit: bool,
    udp_rx: Arc<Mutex<Option<Receiver<UdpStats>>>>,
    cpal_rx: Arc<Mutex<Option<Receiver<CpalStats>>>>,
    device_name: String,
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
        if let Ok(avail) = event::poll(Duration::from_millis(50)) {
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
        let title = Line::from(" Audio Sender ".bold());
        let instructions = Line::from(vec![" Quit ".into(), "<Q> ".blue().bold()]);
        let block = Block::bordered()
            .title(title.centered())
            .title_bottom(instructions.centered())
            .border_set(border::THICK);

        let udp_rx = self.udp_rx.lock().unwrap();
        let cpal_rx = self.cpal_rx.lock().unwrap();

        let udp_stats = udp_rx
            .as_ref()
            .expect("no receiver set")
            .recv_timeout(Duration::from_millis(50));

        let cpal_stats = cpal_rx
            .as_ref()
            .expect("no receiver set")
            .recv_timeout(Duration::from_millis(50));

        let mut occupied = 0;
        let mut received = 0;
        if let Ok(stats) = udp_stats {
            //counter_text = format!("Occupied Size: {}, Received Samples: {}", stats.occupied_buffer, stats.received);
            occupied = stats.occupied_buffer;
            received = stats.sent;
        }

        let mut requested_sample_length = 0;
        if let Ok(stats) = cpal_stats {
            requested_sample_length = stats.requested_sample_length;
        }

        let counter_text = vec![
            Line::from(format!("Input Device Name: {}", self.device_name)),
            Line::from(format!("Occupied Size: {}", occupied)),
            Line::from(format!("Sent Samples: {}", received)),
            Line::from(format!("Requested Samples: {}", requested_sample_length)),
        ];

        Paragraph::new(counter_text).block(block).render(area, buf);
    }
}
