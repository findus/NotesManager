extern crate apple_notes_rs_lib;
extern crate itertools;

use std::{io, thread};
use tui::Terminal;
use tui::backend::CrosstermBackend;
use tui::layout::{Layout, Constraint, Direction};
use tui::widgets::{Borders, Block, ListItem, List, ListState, Paragraph, Wrap};
use tui::style::{Modifier, Style, Color};
use std::sync::mpsc;
use std::time::{Instant, Duration};
use itertools::*;

use crossterm::{
    event::{self, Event as CEvent, KeyCode},
    terminal::{disable_raw_mode, enable_raw_mode},
};
use apple_notes_rs_lib::db::DatabaseService;
use apple_notes_rs_lib::notes::traits::identifyable_note::IdentifyableNote;
use crossterm::event::KeyEvent;
use tui::layout::Alignment;
use apple_notes_rs_lib::notes::localnote::LocalNote;

enum Event<I> {
    Input(I),
    Tick,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    enable_raw_mode().expect("can run in raw mode");
    let stdout = io::stdout();
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    terminal.clear();

    let (tx, rx) = mpsc::channel();
    let tick_rate = Duration::from_millis(10000);

    thread::spawn(move || {
        let mut last_tick = Instant::now();
        loop {
            let timeout = tick_rate
                .checked_sub(last_tick.elapsed())
                .unwrap_or_else(|| Duration::from_secs(0));

            if event::poll(timeout).expect("poll works") {
                if let CEvent::Key(key) = event::read().expect("can read events") {
                    tx.send(Event::Input(key)).expect("can send events");
                }
            }

            if last_tick.elapsed() >= tick_rate {
                if let Ok(_) = tx.send(Event::Tick) {
                    last_tick = Instant::now();
                }
            }
        }
    });


    let db =  apple_notes_rs_lib::db::SqliteDBConnection::new();


    let entries: Vec<LocalNote> = db.fetch_all_notes().unwrap()
        .into_iter()
        .sorted_by_key(|note| format!("{}_{}",&note.metadata.subfolder, &note.body[0].subject()))
        .collect();


    let items: Vec<ListItem> = entries.iter().map(|e| ListItem::new(format!("{} {}",e.metadata.folder(),e.first_subject()))).collect();

    let list = List::new(items.clone())
        .block(Block::default().title("List").borders(Borders::ALL))
        .style(Style::default().fg(Color::White))
        .highlight_style(Style::default().add_modifier(Modifier::ITALIC))
        .highlight_symbol(">>");

    let mut text: String = "".to_string();

    let mut note_list_state = ListState::default();
    note_list_state.select(Some(0));

    loop {
        terminal.draw(|f| {
            let chunks = Layout::default()
                .direction(Direction::Horizontal)
                .margin(1)
                .constraints(
                    [
                        Constraint::Percentage(20),
                        Constraint::Percentage(80),
                    ].as_ref()
                )
                .split(f.size());

            f.render_stateful_widget(list.clone(), chunks[0], &mut note_list_state);

            let t  = Paragraph::new(text.clone())
                .block(Block::default().title("Paragraph").borders(Borders::ALL))
                .style(Style::default().fg(Color::White))
                .alignment(Alignment::Left)
                .wrap(Wrap { trim: true });

            f.render_widget(t, chunks[1]);
        });

        match rx.recv()? {
            Event::Input(event) => match event.code {
                KeyCode::Char('j') => {
                    note_list_state.select(Some(note_list_state.selected().unwrap_or(0) + 1));
                    text = entries.get(note_list_state.selected().unwrap()).unwrap().body[0].text.as_ref().unwrap().clone();
                },
                KeyCode::Char('k') => {
                    note_list_state.select(Some(note_list_state.selected().unwrap_or(0) - 1));
                    text = entries.get(note_list_state.selected().unwrap()).unwrap().body[0].text.as_ref().unwrap().clone();
                },
                KeyCode::Char('q') => {
                    terminal.clear();
                    break;
                }
                _ => {}
            }
            Event::Tick => {}
        }
    }





    Ok(())
}