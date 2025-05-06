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
    text::Line,
    widgets::{Block, Paragraph, Widget},
};

use crate::{components::{cpal::CpalStats, udp::NetworkUDPStats}, AppDebug, Direction};

pub fn tui(
    direction: Direction,
    device_name: String,
    app_debug: AppDebug,
) -> io::Result<()> {
    let mut terminal = ratatui::init();

    let mut app = TuiApp {
        exit: false,
        net_stats: Arc::new(Mutex::new(app_debug.udp_stats_receiver)),
        cpal_stats: Arc::new(Mutex::new(app_debug.cpal_stats_receiver)),
        direction,
        device_name,
    };

    //app.set_receiver(rx, cpal_rx);
    //app.device_name = device.name().unwrap_or("Unknown Device Name".into());

    let app_result = app.run(&mut terminal);
    ratatui::restore();
    app_result
}

#[derive(Debug)]
pub struct TuiApp {
    exit: bool,
    net_stats: Arc<Mutex<Receiver<NetworkUDPStats>>>,
    cpal_stats: Arc<Mutex<Receiver<CpalStats>>>,
    device_name: String,
    direction: Direction,
}

impl TuiApp {
    /// runs the application's main loop until the user quits
    pub fn run(&mut self, terminal: &mut DefaultTerminal) -> io::Result<()> {
        while !self.exit {
            terminal.draw(|frame| self.draw(frame))?;
            self.handle_events()?;
        }
        Ok(())
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
            _ => {}
        }
    }

    fn exit(&mut self) {
        self.exit = true;
    }
}

impl Widget for &TuiApp {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let title_text = match self.direction {
            Direction::Sender => " Audio Sender ",
            Direction::Receiver => " Audio Receiver ",
        };

        let title = Line::from(title_text.bold());
        let instructions = Line::from(vec![" Quit ".into(), "<Q> ".blue().bold()]);

        let block = Block::bordered()
            .title(title.centered())
            .title_bottom(instructions.centered())
            .border_set(border::THICK);

        let net_stats = self.net_stats.lock().unwrap();
        let cpal_stats = self.cpal_stats.lock().unwrap();

        let (net_stats, cpal_stats) = (
            net_stats
                .recv_timeout(Duration::from_millis(250))
                .unwrap_or(NetworkUDPStats::default()),
            cpal_stats
                .recv_timeout(Duration::from_millis(250))
                .unwrap_or(CpalStats::default()),
        );

        let main_text = match self.direction {
            Direction::Sender => {
                vec![
                    Line::from(format!("Input Device Name: {}", self.device_name)),
                    Line::from(format!(
                        "Occupied UDP Buffer Size: {}",
                        net_stats.pre_occupied_buffer
                    )),
                    Line::from(format!("Sent Samples: {}", net_stats.received.unwrap_or(0))),
                    Line::from(format!(
                        "Requested Samples: {}",
                        cpal_stats.requested.unwrap_or(0)
                    )),
                ]
            }
            Direction::Receiver => {
                vec![
                    Line::from(format!("Output Device Name: {}", self.device_name)),
                    Line::from(format!(
                        "Occupied UDP Buffer Size: {}",
                        net_stats.pre_occupied_buffer
                    )),
                    Line::from(format!(
                        "Received Samples: {}",
                        net_stats.received.unwrap_or(0)
                    )),
                    Line::from(format!(
                        "Requested Samples: {}",
                        cpal_stats.requested.unwrap_or(0)
                    )),
                ]
            }
        };

        Paragraph::new(main_text).block(block).render(area, buf);
    }
}
