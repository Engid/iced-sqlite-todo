use iced::mouse;
use iced::widget::{canvas, canvas::Path};
use iced::{Color, Element, Length, Point, Rectangle, Renderer, Size, Theme};

// ---------------------------------------------------------------------------
// Domain types — these live in your app state, not inside the widget
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NoteId(pub usize);

#[derive(Debug, Clone)]
pub struct Note {
    pub id: NoteId,
    pub pitch: u8,        // MIDI note number (or your own pitch repr)
    pub start: f32,       // in beats
    pub duration: f32,    // in beats
    pub velocity: f32,
}

/// Grid configuration — zoom, scroll, quantization
#[derive(Debug, Clone)]
pub struct GridConfig {
    pub beats_visible: f32,       // horizontal zoom (how many beats fit on screen)
    pub lowest_pitch: u8,         // bottom of the visible range
    pub highest_pitch: u8,        // top of the visible range
    pub scroll_x: f32,            // horizontal scroll offset in beats
    pub snap_division: f32,       // quantize to 1/4, 1/8, 1/16 etc.
}

// ---------------------------------------------------------------------------
// Messages the widget emits — the parent app handles these
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub enum PianoGridMessage {
    NoteCreated { pitch: u8, start: f32, duration: f32 },
    NoteMoved { id: NoteId, new_pitch: u8, new_start: f32 },
    NoteResized { id: NoteId, new_duration: f32 },
    NoteSelected(NoteId),
    SelectionCleared,
    Scrolled { delta_x: f32, delta_y: f32 },
}

// ---------------------------------------------------------------------------
// Internal interaction state — managed by the widget tree, not your app
// ---------------------------------------------------------------------------

#[derive(Debug, Default)]
pub struct PianoGridState {
    drag: Option<DragAction>,
}

#[derive(Debug, Clone)]
enum DragAction {
    /// Creating a new note by clicking empty space and dragging
    Creating { pitch: u8, start_beat: f32, current_beat: f32 },
    /// Moving an existing note
    Moving {
        id: NoteId,
        grab_offset_beats: f32,
        pitch_offset: i16,
    },
    /// Resizing from the right edge
    Resizing { id: NoteId, start_beat: f32 },
}

// ---------------------------------------------------------------------------
// The widget itself — borrows data, doesn't own it
// ---------------------------------------------------------------------------

pub struct PianoGrid<'a> {
    notes: &'a [Note],
    config: &'a GridConfig,
    selected: Option<NoteId>,
}

impl<'a> PianoGrid<'a> {
    pub fn new(
        notes: &'a [Note],
        config: &'a GridConfig,
        selected: Option<NoteId>,
    ) -> Self {
        Self { notes, config, selected }
    }

    // -- coordinate conversion helpers --

    fn beat_to_x(&self, beat: f32, bounds: Rectangle) -> f32 {
        let beats_offset = beat - self.config.scroll_x;
        let pixels_per_beat = bounds.width / self.config.beats_visible;
        bounds.x + beats_offset * pixels_per_beat
    }

    fn x_to_beat(&self, x: f32, bounds: Rectangle) -> f32 {
        let pixels_per_beat = bounds.width / self.config.beats_visible;
        self.config.scroll_x + (x - bounds.x) / pixels_per_beat
    }

    fn pitch_count(&self) -> f32 {
        (self.config.highest_pitch - self.config.lowest_pitch + 1) as f32
    }

    fn row_height(&self, bounds: Rectangle) -> f32 {
        bounds.height / self.pitch_count().max(1.0)
    }

    fn pitch_to_y(&self, pitch: u8, bounds: Rectangle) -> f32 {
        let row_height = self.row_height(bounds);
        // higher pitches at the top
        let offset = (self.config.highest_pitch - pitch) as f32;
        bounds.y + offset * row_height
    }

    fn y_to_pitch(&self, y: f32, bounds: Rectangle) -> u8 {
        let row_height = self.row_height(bounds);
        let relative_y = (y - bounds.y).clamp(0.0, (bounds.height - 1.0).max(0.0));
        let offset = (relative_y / row_height).floor() as i16;
        let highest = self.config.highest_pitch as i16;
        let lowest = self.config.lowest_pitch as i16;

        (highest - offset).clamp(lowest, highest) as u8
    }

    fn note_rect(&self, note: &Note, bounds: Rectangle) -> Rectangle {
        let row_height = self.row_height(bounds);

        Rectangle {
            x: self.beat_to_x(note.start, bounds),
            y: self.pitch_to_y(note.pitch, bounds),
            width: note.duration * (bounds.width / self.config.beats_visible),
            height: row_height,
        }
    }

    fn snap(&self, beat: f32) -> f32 {
        let div = self.config.snap_division;
        (beat / div).round() * div
    }

    fn hit_test(&self, position: Point, bounds: Rectangle) -> Option<(NoteId, HitZone)> {
        let resize_margin = 6.0;

        // iterate in reverse so topmost (last-drawn) notes are hit first
        for note in self.notes.iter().rev() {
            let rect = self.note_rect(note, bounds);
            if rect.contains(position) {
                let zone = if position.x > rect.x + rect.width - resize_margin {
                    HitZone::RightEdge
                } else {
                    HitZone::Body
                };
                return Some((note.id, zone));
            }
        }
        None
    }

    fn local_bounds(&self, size: Size) -> Rectangle {
        Rectangle {
            x: 0.0,
            y: 0.0,
            width: size.width,
            height: size.height,
        }
    }

    fn preview_rect(&self, state: &PianoGridState, bounds: Rectangle) -> Option<Rectangle> {
        let DragAction::Creating {
            pitch,
            start_beat,
            current_beat,
        } = state.drag.as_ref()? else {
            return None;
        };

        let (start, end) = if start_beat <= current_beat {
            (*start_beat, *current_beat)
        } else {
            (*current_beat, *start_beat)
        };

        Some(Rectangle {
            x: self.beat_to_x(start, bounds),
            y: self.pitch_to_y(*pitch, bounds),
            width: ((end - start).max(self.config.snap_division))
                * (bounds.width / self.config.beats_visible),
            height: self.row_height(bounds),
        })
    }
}

#[derive(Debug, Clone, Copy)]
enum HitZone {
    Body,
    RightEdge,
}

// ---------------------------------------------------------------------------
// Widget trait implementation
// ---------------------------------------------------------------------------

impl<'a> canvas::Program<PianoGridMessage, Theme, Renderer> for PianoGrid<'a> {
    type State = PianoGridState;

    fn draw(
        &self,
        state: &Self::State,
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<canvas::Geometry<Renderer>> {
        let local_bounds = self.local_bounds(bounds.size());
        let mut frame = canvas::Frame::new(renderer, bounds.size());
        let row_height = self.row_height(local_bounds);

        for i in 0..self.pitch_count() as u8 {
            let pitch = self.config.highest_pitch.saturating_sub(i);
            let y = local_bounds.y + (i as f32) * row_height;
            let is_black_key = matches!(pitch % 12, 1 | 3 | 6 | 8 | 10);
            let color = if is_black_key {
                Color::from_rgb(0.11, 0.11, 0.13)
            } else {
                Color::from_rgb(0.15, 0.15, 0.18)
            };

            frame.fill(
                &Path::rectangle(
                    Point::new(local_bounds.x, y),
                    Size::new(local_bounds.width, row_height),
                ),
                color,
            );
        }

        let first_division =
            (self.config.scroll_x / self.config.snap_division).floor() as i32;
        let last_division = ((self.config.scroll_x + self.config.beats_visible)
            / self.config.snap_division)
            .ceil() as i32;

        for division in first_division..=last_division {
            let beat = division as f32 * self.config.snap_division;
            let x = self.beat_to_x(beat, local_bounds);
            let color = if division % ((1.0 / self.config.snap_division).round() as i32).max(1)
                == 0
            {
                if (beat as i32) % 4 == 0 {
                    Color::from_rgba(1.0, 1.0, 1.0, 0.20)
                } else {
                    Color::from_rgba(1.0, 1.0, 1.0, 0.10)
                }
            } else {
                Color::from_rgba(1.0, 1.0, 1.0, 0.05)
            };

            frame.fill(
                &Path::rectangle(
                    Point::new(x, local_bounds.y),
                    Size::new(1.0, local_bounds.height),
                ),
                color,
            );
        }

        for note in self.notes {
            let rect = self.note_rect(note, local_bounds);
            let velocity_tint = note.velocity.clamp(0.2, 1.0);
            let note_color = if self.selected == Some(note.id) {
                Color::from_rgb(0.25, 0.55 + 0.25 * velocity_tint, 0.95)
            } else {
                Color::from_rgb(0.18, 0.35 + 0.20 * velocity_tint, 0.72)
            };

            frame.fill(
                &Path::rectangle(
                    Point::new(rect.x, rect.y + 1.0),
                    Size::new(rect.width.max(2.0), (rect.height - 2.0).max(2.0)),
                ),
                note_color,
            );
        }

        if let Some(preview) = self.preview_rect(state, local_bounds) {
            frame.fill(
                &Path::rectangle(
                    Point::new(preview.x, preview.y + 1.0),
                    Size::new(preview.width.max(2.0), (preview.height - 2.0).max(2.0)),
                ),
                Color::from_rgba(0.65, 0.82, 1.0, 0.45),
            );
        }

        vec![frame.into_geometry()]
    }

    fn update(
        &self,
        state: &mut Self::State,
        event: &canvas::Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<canvas::Action<PianoGridMessage>> {
        let local_bounds = self.local_bounds(bounds.size());
        let position = cursor.position_in(bounds);

        match event {
            canvas::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                let position = position?;

                match self.hit_test(position, local_bounds) {
                    Some((id, HitZone::Body)) => {
                        let note = self.notes.iter().find(|note| note.id == id)?;
                        let cursor_pitch = self.y_to_pitch(position.y, local_bounds) as i16;
                        state.drag = Some(DragAction::Moving {
                            id,
                            grab_offset_beats: self.x_to_beat(position.x, local_bounds)
                                - note.start,
                            pitch_offset: note.pitch as i16 - cursor_pitch,
                        });

                        Some(canvas::Action::publish(PianoGridMessage::NoteSelected(id)).and_capture())
                    }
                    Some((id, HitZone::RightEdge)) => {
                        let note = self.notes.iter().find(|note| note.id == id)?;
                        state.drag = Some(DragAction::Resizing {
                            id,
                            start_beat: note.start,
                        });

                        Some(canvas::Action::publish(PianoGridMessage::NoteSelected(id)).and_capture())
                    }
                    None => {
                        let pitch = self.y_to_pitch(position.y, local_bounds);
                        let beat = self.snap(self.x_to_beat(position.x, local_bounds));
                        state.drag = Some(DragAction::Creating {
                            pitch,
                            start_beat: beat,
                            current_beat: beat,
                        });

                        Some(
                            canvas::Action::publish(PianoGridMessage::SelectionCleared)
                                .and_capture(),
                        )
                    }
                }
            }
            canvas::Event::Mouse(mouse::Event::CursorMoved { .. }) => match &mut state.drag {
                Some(DragAction::Moving {
                    id,
                    grab_offset_beats,
                    pitch_offset,
                }) => {
                    let cursor_beat = self.x_to_beat(position?.x, local_bounds);
                    let cursor_pitch = self.y_to_pitch(position?.y, local_bounds) as i16;
                    let new_pitch = (cursor_pitch + *pitch_offset).clamp(
                        self.config.lowest_pitch as i16,
                        self.config.highest_pitch as i16,
                    ) as u8;
                    let new_start = self.snap((cursor_beat - *grab_offset_beats).max(0.0));

                    Some(
                        canvas::Action::publish(PianoGridMessage::NoteMoved {
                            id: *id,
                            new_pitch,
                            new_start,
                        })
                        .and_capture(),
                    )
                }
                Some(DragAction::Resizing { id, start_beat }) => {
                    let cursor_beat = self.x_to_beat(position?.x, local_bounds);
                    let duration = self.snap((cursor_beat - *start_beat).max(self.config.snap_division));

                    Some(
                        canvas::Action::publish(PianoGridMessage::NoteResized {
                            id: *id,
                            new_duration: duration,
                        })
                        .and_capture(),
                    )
                }
                Some(DragAction::Creating { current_beat, .. }) => {
                    *current_beat = self.snap(self.x_to_beat(position?.x, local_bounds));
                    Some(canvas::Action::request_redraw().and_capture())
                }
                None => None,
            },
            canvas::Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => {
                match state.drag.take() {
                    Some(DragAction::Creating {
                        pitch,
                        start_beat,
                        current_beat,
                    }) => {
                        let (start, end) = if start_beat <= current_beat {
                            (start_beat, current_beat)
                        } else {
                            (current_beat, start_beat)
                        };

                        Some(
                            canvas::Action::publish(PianoGridMessage::NoteCreated {
                                pitch,
                                start,
                                duration: (end - start).max(self.config.snap_division),
                            })
                            .and_capture(),
                        )
                    }
                    Some(_) => Some(canvas::Action::capture()),
                    None => None,
                }
            }
            canvas::Event::Mouse(mouse::Event::WheelScrolled { delta }) => {
                let (delta_x, delta_y) = match delta {
                    mouse::ScrollDelta::Lines { x, y } => (*x, *y),
                    mouse::ScrollDelta::Pixels { x, y } => (*x / 32.0, *y / 32.0),
                };

                Some(
                    canvas::Action::publish(PianoGridMessage::Scrolled { delta_x, delta_y })
                        .and_capture(),
                )
            }
            _ => None,
        }
    }

    fn mouse_interaction(
        &self,
        state: &Self::State,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> mouse::Interaction {
        let local_bounds = self.local_bounds(bounds.size());

        match &state.drag {
            Some(DragAction::Moving { .. }) => return mouse::Interaction::Grabbing,
            Some(DragAction::Resizing { .. }) => {
                return mouse::Interaction::ResizingHorizontally;
            }
            Some(DragAction::Creating { .. }) => return mouse::Interaction::Crosshair,
            None => {}
        }

        if let Some(position) = cursor.position_in(bounds) {
            match self.hit_test(position, local_bounds) {
                Some((_, HitZone::RightEdge)) => mouse::Interaction::ResizingHorizontally,
                Some((_, HitZone::Body)) => mouse::Interaction::Grab,
                None => mouse::Interaction::Crosshair,
            }
        } else {
            mouse::Interaction::default()
        }
    }
}

// ---------------------------------------------------------------------------
// Convenience: into Element
// ---------------------------------------------------------------------------

impl<'a> From<PianoGrid<'a>> for Element<'a, PianoGridMessage> {
    fn from(grid: PianoGrid<'a>) -> Self {
        iced::widget::Canvas::new(grid)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }
}

pub fn view<'a>(
    notes: &'a [Note],
    config: &'a GridConfig,
    selected: Option<NoteId>,
) -> Element<'a, PianoGridMessage> {
    PianoGrid::new(notes, config, selected).into()
}

// ---------------------------------------------------------------------------
// Usage in your app's view function would look roughly like:
// ---------------------------------------------------------------------------
//
// fn view(&self) -> Element<AppMessage> {
//     let grid = PianoGrid::new(
//         &self.notes,
//         &self.grid_config,
//         self.selected_note,
//     );
//
//     // map the widget's messages into your app's message type
//     Element::from(grid).map(AppMessage::PianoGrid)
// }