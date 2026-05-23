use iced::widget::{
    button, checkbox, container, row, scrollable,
    text, text_input, Column,
};
use iced::{Center, Element, Fill, Task, Theme};

use crate::db::{Database, Todo};
use crate::piano_grid::{self, GridConfig, Note, NoteId, PianoGridMessage};

// ── State ──────────────────────────────────────────────────────────────

pub struct TodoApp {
    db: Database,
    todos: Vec<Todo>,
    input: String,
    status: Option<String>,
    grid_notes: Vec<Note>,
    grid_config: GridConfig,
    selected_note: Option<NoteId>,
    next_note_id: usize,
}

// ── Messages ───────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum Message {
    InputChanged(String),
    AddTodo,
    Toggle(i64),
    Delete(i64),
    PianoGrid(PianoGridMessage),
}

// ── Boot / Update / View ───────────────────────────────────────────────

impl TodoApp {
    /// Boot function — called once by iced to create initial state.
    /// On wasm, the OPFS VFS is already installed by the time this runs
    /// (see main.rs), so `Database::open` goes through the OPFS backend.
    pub fn new() -> (Self, Task<Message>) {
        let db = Database::open("todos.db").unwrap_or_else(|e| {
            eprintln!("File DB failed ({e}), falling back to in-memory");
            Database::open_in_memory().expect("in-memory DB failed")
        });
        let todos = db.list().unwrap_or_default();

        let app = Self {
            db,
            todos,
            input: String::new(),
            status: None,
            grid_notes: vec![
                Note {
                    id: NoteId(0),
                    pitch: 64,
                    start: 0.0,
                    duration: 1.0,
                    velocity: 0.9,
                },
                Note {
                    id: NoteId(1),
                    pitch: 67,
                    start: 1.0,
                    duration: 1.5,
                    velocity: 0.75,
                },
                Note {
                    id: NoteId(2),
                    pitch: 71,
                    start: 2.5,
                    duration: 0.75,
                    velocity: 0.8,
                },
            ],
            grid_config: GridConfig {
                beats_visible: 16.0,
                lowest_pitch: 48,
                highest_pitch: 72,
                scroll_x: 0.0,
                snap_division: 0.25,
            },
            selected_note: Some(NoteId(1)),
            next_note_id: 3,
        };
        (app, Task::none())
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::InputChanged(value) => {
                self.input = value;
            }
            Message::AddTodo => {
                let title = self.input.trim().to_string();
                if !title.is_empty() {
                    match self.db.add(&title) {
                        Ok(_) => {
                            self.input.clear();
                            self.refresh();
                        }
                        Err(e) => self.status = Some(format!("Insert failed: {e}")),
                    }
                }
            }
            Message::Toggle(id) => {
                if let Err(e) = self.db.toggle(id) {
                    self.status = Some(format!("Toggle failed: {e}"));
                } else {
                    self.refresh();
                }
            }
            Message::Delete(id) => {
                if let Err(e) = self.db.delete(id) {
                    self.status = Some(format!("Delete failed: {e}"));
                } else {
                    self.refresh();
                }
            }
            Message::PianoGrid(message) => self.update_piano_grid(message),
        }
        Task::none()
    }

    pub fn view(&self) -> Element<'_, Message> {
        let todo_panel = self.todo_panel();

        let selected_note = self.selected_note.and_then(|id| {
            self.grid_notes
                .iter()
                .find(|note| note.id == id)
                .map(|note| {
                    format!(
                        "Selected note: pitch {} | start {:.2} | duration {:.2}",
                        note.pitch, note.start, note.duration
                    )
                })
        });

        let mut piano_panel = Column::new()
            .spacing(12)
            .push(text("Piano Grid Experiment").size(24))
            .push(text("Drag notes to move them, drag the right edge to resize, and drag empty space to create new notes.").size(14));

        if let Some(summary) = selected_note {
            piano_panel = piano_panel.push(text(summary).size(13));
        }

        piano_panel = piano_panel.push(
            container(
                piano_grid::view(&self.grid_notes, &self.grid_config, self.selected_note)
                    .map(Message::PianoGrid),
            )
            .height(Fill)
            .padding(8),
        );

        container(
            row![
                container(todo_panel).width(Fill),
                container(piano_panel).width(Fill),
            ]
            .spacing(16)
            .height(Fill),
        )
            .padding(10)
            .into()
    }

    pub fn theme(&self) -> Theme {
        Theme::Dark
    }

    // ── helpers ────────────────────────────────────────────────────

    fn refresh(&mut self) {
        match self.db.list() {
            Ok(t) => {
                self.todos = t;
                self.status = None;
            }
            Err(e) => self.status = Some(format!("DB error: {e}")),
        }
    }

    fn todo_panel(&self) -> Column<'_, Message> {
        let title = text("✅ Todos (iced + rusqlite)").size(24);

        let counter = match self.db.count() {
            Ok((done, total)) => text(format!("{done}/{total} completed")).size(14),
            Err(_) => text("").size(14),
        };

        let input = text_input("What needs to be done?", &self.input)
            .on_input(Message::InputChanged)
            .on_submit(Message::AddTodo)
            .padding(10);

        let add_btn = button("Add")
            .on_press(Message::AddTodo)
            .padding([8, 16]);

        let input_row = row![input, add_btn].spacing(10);

        let mut content = Column::new()
            .spacing(12)
            .padding(20)
            .push(title)
            .push(counter)
            .push(input_row);

        if let Some(msg) = &self.status {
            content = content.push(
                text(msg.clone())
                    .color([1.0, 0.3, 0.3])
                    .size(13),
            );
        }

        let todos: Column<Message> = self
            .todos
            .iter()
            .fold(Column::new().spacing(6), |col, todo| col.push(todo_row(todo)));

        content.push(scrollable(todos).height(Fill))
    }

    fn update_piano_grid(&mut self, message: PianoGridMessage) {
        match message {
            PianoGridMessage::NoteCreated {
                pitch,
                start,
                duration,
            } => {
                let id = NoteId(self.next_note_id);
                self.next_note_id += 1;
                self.grid_notes.push(Note {
                    id,
                    pitch,
                    start: start.max(0.0),
                    duration: duration.max(self.grid_config.snap_division),
                    velocity: 0.8,
                });
                self.selected_note = Some(id);
            }
            PianoGridMessage::NoteMoved {
                id,
                new_pitch,
                new_start,
            } => {
                if let Some(note) = self.grid_notes.iter_mut().find(|note| note.id == id) {
                    note.pitch = new_pitch
                        .clamp(self.grid_config.lowest_pitch, self.grid_config.highest_pitch);
                    note.start = new_start.max(0.0);
                    self.selected_note = Some(id);
                }
            }
            PianoGridMessage::NoteResized { id, new_duration } => {
                if let Some(note) = self.grid_notes.iter_mut().find(|note| note.id == id) {
                    note.duration = new_duration.max(self.grid_config.snap_division);
                    self.selected_note = Some(id);
                }
            }
            PianoGridMessage::NoteSelected(id) => {
                self.selected_note = Some(id);
            }
            PianoGridMessage::SelectionCleared => {
                self.selected_note = None;
            }
            PianoGridMessage::Scrolled { delta_x, delta_y } => {
                self.grid_config.scroll_x =
                    (self.grid_config.scroll_x - delta_x * self.grid_config.snap_division).max(0.0);
                self.shift_pitch_window((-delta_y.round()) as i16);
            }
        }
    }

    fn shift_pitch_window(&mut self, delta: i16) {
        if delta == 0 {
            return;
        }

        let span = self.grid_config.highest_pitch as i16 - self.grid_config.lowest_pitch as i16;
        let mut lowest = self.grid_config.lowest_pitch as i16 + delta;
        let mut highest = lowest + span;

        if lowest < 0 {
            lowest = 0;
            highest = span;
        }

        if highest > 127 {
            highest = 127;
            lowest = highest - span;
        }

        self.grid_config.lowest_pitch = lowest as u8;
        self.grid_config.highest_pitch = highest as u8;
    }
}

/// Build the widget row for a single todo item.
fn todo_row(todo: &Todo) -> Element<'_, Message> {
    let cb = checkbox(todo.completed)
        .on_toggle(move |_| Message::Toggle(todo.id));

    let label = if todo.completed {
        text(&todo.title).color([0.6, 0.6, 0.6])
    } else {
        text(&todo.title)
    };

    let delete = button("🗑")
        .on_press(Message::Delete(todo.id))
        .padding([4, 8]);

    row![cb, container(label).width(Fill), delete]
        .spacing(10)
        .align_y(Center)
        .into()
}
