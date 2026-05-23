# todo-iced-sqlite

Same PoC as `todo-egui-sqlite`, but using **iced 0.14** (Elm architecture)
instead of egui (immediate mode).

| Platform | GUI   | SQLite backend  | Storage         |
|----------|-------|-----------------|-----------------|
| Desktop  | iced  | libsqlite3-sys  | `todos.db` file |
| Browser  | iced  | sqlite-wasm-rs  | OPFS (SAH-pool) |

## Architecture differences from the egui version

The `db.rs` module is **identical** — same `Database` struct, same CRUD methods,
same `rusqlite::Connection` under the hood.  The only differences are in the
UI layer and entry point:

- **iced uses the Elm architecture**: a `Message` enum, an `update` function
  that pattern-matches on messages and returns a `Task`, and a `view` function
  that builds a widget tree from state.  No mutable borrows during rendering.
- **egui is immediate mode**: you mutate state directly inside `ui.button().clicked()`
  callbacks during the same frame.

Both approaches work fine for a CRUD todo app.  The Elm architecture pays off
more as apps grow — undo/redo, time-travel debugging, and testability come
naturally from the message/update separation.

## Prerequisites

```
rustup target add wasm32-unknown-unknown
cargo install trunk
```

## Build & run

### Desktop

```bash
cargo run
```

### Browser

```bash
trunk serve
```

Opens `http://127.0.0.1:8080`.

### Release build

```bash
trunk build --release
```

## Project structure

```
Cargo.toml          Platform-conditional deps (iced + webgl for wasm)
index.html          Trunk entry point
src/
  main.rs           Entry points — OPFS init on wasm, then iced::application()
  app.rs            Message / update / view — the Elm architecture
  db.rs             Identical rusqlite wrapper from the egui version
```

## Notes

- **iced on wasm uses WebGL** (via the `webgl` feature).  The default wgpu
  WebGPU backend isn't universally available yet, so WebGL is the safer bet.
  You can swap to `"wgpu"` once WebGPU lands in more browsers.

- The wasm entry point uses `spawn_local` to run OPFS init before starting
  iced.  This works because on wasm, `iced::application().run()` sets up
  requestAnimationFrame callbacks and returns immediately.

- Same caveats as the egui version: `sqlite-wasm-rs` is single-threaded
  (`SQLITE_THREADSAFE=0`), SAH-pool pre-allocates file handles, and no
  WAL mode on OPFS.

## License

Public domain / MIT — do whatever you want with it.
