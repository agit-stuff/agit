//! Event handling for TUI.
//!
//! Manages keyboard input, resize events, and periodic ticks.

use crossterm::event::{self, Event as CrosstermEvent, KeyEvent, KeyEventKind};
use std::time::Duration;
use tokio::sync::mpsc;

/// TUI events that can occur.
#[derive(Debug, Clone)]
pub enum Event {
    /// Key press event
    Key(KeyEvent),
    /// Terminal resize event
    Resize(u16, u16),
    /// Periodic tick for UI updates
    Tick,
    /// Request to refresh data from storage
    Refresh,
}

/// Event handler that manages multiple event sources.
pub struct EventHandler {
    rx: mpsc::UnboundedReceiver<Event>,
    _tx: mpsc::UnboundedSender<Event>,
}

impl EventHandler {
    /// Create a new event handler.
    pub fn new(tick_rate: Duration) -> Self {
        let (tx, rx) = mpsc::unbounded_channel();

        // Spawn event listener thread for keyboard/mouse events
        let event_tx = tx.clone();
        std::thread::spawn(move || {
            loop {
                // Poll for events with a timeout
                if event::poll(Duration::from_millis(100)).unwrap_or(false) {
                    match event::read() {
                        Ok(CrosstermEvent::Key(key)) => {
                            // Only handle key press events (not release)
                            if key.kind == KeyEventKind::Press {
                                if event_tx.send(Event::Key(key)).is_err() {
                                    break;
                                }
                            }
                        }
                        Ok(CrosstermEvent::Resize(w, h)) => {
                            if event_tx.send(Event::Resize(w, h)).is_err() {
                                break;
                            }
                        }
                        _ => {}
                    }
                }
            }
        });

        // Spawn tick generator thread
        let tick_tx = tx.clone();
        std::thread::spawn(move || {
            loop {
                std::thread::sleep(tick_rate);
                if tick_tx.send(Event::Tick).is_err() {
                    break;
                }
            }
        });

        Self { rx, _tx: tx }
    }

    /// Get the next event (blocking).
    pub async fn next(&mut self) -> Option<Event> {
        self.rx.recv().await
    }
}

impl Default for EventHandler {
    fn default() -> Self {
        Self::new(Duration::from_millis(250))
    }
}
