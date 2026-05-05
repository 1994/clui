use crossterm::event::{self, Event as CrosstermEvent, KeyEvent, MouseEvent};
use std::time::{Duration, Instant};
use tokio::sync::mpsc;

#[derive(Debug, Clone, Copy)]
pub enum Event {
    Tick,
    Key(KeyEvent),
    Mouse(MouseEvent),
    Resize(#[allow(dead_code)] u16, #[allow(dead_code)] u16),
}

pub struct EventHandler {
    rx: mpsc::UnboundedReceiver<Event>,
}

impl EventHandler {
    pub fn new(tick_rate: Duration) -> Self {
        let (tx, rx) = mpsc::unbounded_channel();

        std::thread::spawn(move || {
            let mut last_tick = Instant::now();
            loop {
                let timeout = tick_rate
                    .checked_sub(last_tick.elapsed())
                    .unwrap_or_else(|| Duration::from_secs(0));

                let has_input = event::poll(timeout).unwrap_or(false);

                if has_input {
                    match event::read() {
                        Ok(CrosstermEvent::Key(key)) => {
                            if tx.send(Event::Key(key)).is_err() {
                                break;
                            }
                        }
                        Ok(CrosstermEvent::Mouse(mouse)) => {
                            if tx.send(Event::Mouse(mouse)).is_err() {
                                break;
                            }
                        }
                        Ok(CrosstermEvent::Resize(x, y)) => {
                            if tx.send(Event::Resize(x, y)).is_err() {
                                break;
                            }
                        }
                        Ok(_) | Err(_) => {}
                    }
                }

                if last_tick.elapsed() >= tick_rate {
                    if tx.send(Event::Tick).is_err() {
                        break;
                    }
                    last_tick = Instant::now();
                }
            }
        });

        Self { rx }
    }

    pub async fn next(&mut self) -> Option<Event> {
        self.rx.recv().await
    }
}
