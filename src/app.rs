use iced::widget::{
    button, checkbox, container, row, scrollable,
    text, text_input, Column,
};
use iced::{Center, Element, Fill, Task, Theme};

use crate::db::{Database, Todo};

// ── State ──────────────────────────────────────────────────────────────

pub struct TodoApp {
    db: Database,
    todos: Vec<Todo>,
    input: String,
    status: Option<String>,
}

// ── Messages ───────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum Message {
    InputChanged(String),
    AddTodo,
    Toggle(i64),
    Delete(i64),
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
        }
        Task::none()
    }

    pub fn view(&self) -> Element<'_, Message> {
        // ── header ─────────────────────────────────────────────────
        let title = text("✅ Todos (iced + rusqlite)").size(24);

        let counter = match self.db.count() {
            Ok((done, total)) => text(format!("{done}/{total} completed")).size(14),
            Err(_) => text("").size(14),
        };

        // ── input row ──────────────────────────────────────────────
        let input = text_input("What needs to be done?", &self.input)
            .on_input(Message::InputChanged)
            .on_submit(Message::AddTodo)
            .padding(10);

        let add_btn = button("Add")
            .on_press(Message::AddTodo)
            .padding([8, 16]);

        let input_row = row![input, add_btn].spacing(10);

        // ── error banner ───────────────────────────────────────────
        let mut content = Column::new()
            .spacing(12)
            .padding(20)
            .max_width(600)
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

        // ── todo list ──────────────────────────────────────────────
        let todos: Column<Message> = self
            .todos
            .iter()
            .fold(Column::new().spacing(6), |col, todo| {
                col.push(todo_row(todo))
            });

        content = content.push(scrollable(todos).height(Fill));

        container(content)
            .center_x(Fill)
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
