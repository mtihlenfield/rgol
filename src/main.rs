use crossterm::event;
use crossterm::{ExecutableCommand, QueueableCommand, cursor, style, terminal};
use log::{error, info, warn};
use log4rs;
use std::io::{Stdout, Write, stdout};
use std::time::{Duration, Instant};

#[derive(Clone, Copy)]
struct Position {
    col: usize,
    row: usize,
}

#[derive(Clone, PartialEq, Eq)]
enum GameMode {
    Running,
    Editing,
}

struct State {
    map: Vec<Vec<bool>>,
    next_map: Vec<Vec<bool>>,
    mode: GameMode,
    rows: usize,
    cols: usize,
    cursor: Position,
}

impl State {
    pub fn new(cols: usize, rows: usize) -> State {
        State {
            rows,
            cols,
            map: vec![vec![false; cols]; rows],
            next_map: vec![vec![false; cols]; rows],
            cursor: Position {
                col: cols.div_ceil(2),
                row: rows.div_ceil(2),
            },
            mode: GameMode::Editing,
        }
    }

    fn relative_pos(
        &self,
        row: usize,
        col: usize,
        row_move: isize,
        col_move: isize,
    ) -> (usize, usize) {
        let row = row.checked_add_signed(row_move).unwrap_or(self.rows - 1) % self.rows;
        let col = col.checked_add_signed(col_move).unwrap_or(self.cols - 1) % self.cols;

        (row, col)
    }

    fn live_neighbors(&self, row: usize, col: usize) -> usize {
        [
            self.relative_pos(row, col, -1, -1),
            self.relative_pos(row, col, -1, 0),
            self.relative_pos(row, col, -1, 1),
            self.relative_pos(row, col, 0, -1),
            self.relative_pos(row, col, 0, 1),
            self.relative_pos(row, col, 1, -1),
            self.relative_pos(row, col, 1, 0),
            self.relative_pos(row, col, 1, 1),
        ]
        .iter()
        .filter(|pos| self.get_cell(pos.0, pos.1))
        .count()
    }

    fn set_next_cell(&mut self, row: usize, col: usize, state: bool) {
        self.next_map[row][col] = state;
    }

    fn set_cell(&mut self, row: usize, col: usize, state: bool) {
        self.map[row][col] = state;
        self.next_map[row][col] = state;
    }

    fn get_cell(&self, row: usize, col: usize) -> bool {
        self.map[row][col]
    }

    pub fn step(&mut self) {
        // Rules:
        // 1. Any living cell with fewer than two live neighbors dies (underpopulation)
        // 2. Any live cell with two or three live neighbors lives on to the next generation
        // 3. Any live cell with more then three live neighbors dies (overpopulation)
        // 4. Any dead cell with exactly three live neighbors becomes a live cell (reproduction)
        self.next_map.iter_mut().for_each(|row| row.fill(false));
        for row in 0..self.rows {
            for col in 0..self.cols {
                let live = self.get_cell(row, col);
                let live_neighbors = self.live_neighbors(row, col);

                if live && (live_neighbors < 2 || live_neighbors > 3) {
                    self.set_next_cell(row, col, false);
                } else if live_neighbors == 3 {
                    self.set_next_cell(row, col, true);
                } else if live && live_neighbors == 2 {
                    self.set_next_cell(row, col, true);
                }
            }
        }

        self.map.clone_from(&self.next_map);
    }

    pub fn handle_key(&mut self, key_ev: event::KeyEvent) -> bool {
        if key_ev.code.is_char('q') {
            return true;
        }

        match self.mode {
            GameMode::Editing => self.handle_edit_update(key_ev),
            GameMode::Running => self.handle_running_update(key_ev),
        };

        false
    }

    fn handle_edit_update(&mut self, key_ev: event::KeyEvent) {
        match key_ev.code.as_char() {
            Some('s') => self.cursor.row = (self.cursor.row + 1) % self.rows,
            Some('d') => self.cursor.col = (self.cursor.col + 1) % self.cols,
            Some('w') => {
                self.cursor.row = if self.cursor.row == 0 {
                    self.rows - 1
                } else {
                    self.cursor.row - 1
                };
            }
            Some('a') => {
                self.cursor.col = if self.cursor.col == 0 {
                    self.cols - 1
                } else {
                    self.cursor.col - 1
                };
            }
            Some(' ') => self.set_cell(
                self.cursor.row,
                self.cursor.col,
                !self.get_cell(self.cursor.row, self.cursor.col),
            ),
            Some('p') => self.mode = GameMode::Running,
            Some('x') => {
                let live = self.live_neighbors(self.cursor.row, self.cursor.col);
                info!("live neighbors: {}", live);
            }
            Some('r') => {
                self.map.iter_mut().for_each(|row| row.fill(false));
                self.next_map.iter_mut().for_each(|row| row.fill(false));
            }
            Some(_) | None => {}
        };
    }

    fn handle_running_update(&mut self, key_ev: event::KeyEvent) {
        match key_ev.code.as_char() {
            Some('p') => self.mode = GameMode::Editing,
            Some(_) | None => {}
        }
    }

    pub fn resize(&mut self, rows: usize, cols: usize) {
        // TODO
    }
}

struct ScreenBuf {
    rows: u16,
    cols: u16,
    back: Vec<Vec<char>>,
    front: Vec<Vec<char>>,
}

impl ScreenBuf {
    pub fn new(rows: u16, cols: u16) -> ScreenBuf {
        ScreenBuf {
            rows,
            cols,
            back: vec![vec![' '; cols as usize]; rows as usize],
            front: vec![vec![' '; cols as usize]; rows as usize],
        }
    }

    pub fn clear(&mut self) {
        self.back.iter_mut().for_each(|row| row.fill(' '));
    }

    pub fn write(&mut self, row: u16, col: u16, val: char) {
        self.back[row as usize][col as usize] = val;
    }

    pub fn flush(&mut self, out: &mut Stdout) -> std::io::Result<()> {
        for row in 0..self.rows {
            for col in 0..self.cols {
                let front_col = self.front[row as usize][col as usize];
                let back_col = self.back[row as usize][col as usize];
                if back_col != front_col {
                    out.queue(cursor::MoveTo(col, row))?;
                    out.queue(style::Print(back_col))?;
                    self.front[row as usize][col as usize] = back_col.clone();
                }
            }
        }
        Ok(())
    }
}

struct Screen {
    screen_buf: ScreenBuf,
    initialized: bool,
}

impl Screen {
    pub fn new(rows: u16, cols: u16) -> Screen {
        Screen {
            screen_buf: ScreenBuf::new(rows, cols),
            initialized: false,
        }
    }

    pub fn resize(&mut self, rows: u16, cols: u16) -> std::io::Result<()> {
        self.screen_buf = ScreenBuf::new(rows, cols);

        let mut out = stdout();
        out.execute(terminal::Clear(terminal::ClearType::All))?;
        Ok(())
    }

    pub fn update(&mut self, new_state: &State) -> std::io::Result<()> {
        let mut out = stdout();
        if !self.initialized {
            terminal::enable_raw_mode()?;
            out.execute(terminal::EnterAlternateScreen)?;
            out.execute(terminal::Clear(terminal::ClearType::All))?;
            self.initialized = true;
        }

        self.screen_buf.clear();
        self.draw_map(new_state);

        match new_state.mode {
            GameMode::Running => out.queue(cursor::Hide)?,
            GameMode::Editing => {
                out.queue(cursor::Show)?;
                out.queue(cursor::MoveTo(
                    new_state.cursor.col as u16,
                    new_state.cursor.row as u16,
                ))?
            }
        };

        self.screen_buf.flush(&mut out)?;

        out.flush()
    }

    fn draw_map(&mut self, new_state: &State) {
        // TODO: indexing use the new_state rows/cols is a little risky - how can I be sure those
        // line up with the screen buff size?
        for row in 0..new_state.rows {
            for col in 0..new_state.cols {
                if new_state.get_cell(row, col) {
                    self.screen_buf.write(row as u16, col as u16, '0');
                } else {
                    self.screen_buf.write(row as u16, col as u16, '.');
                }
            }
        }
    }
}

impl Drop for Screen {
    fn drop(&mut self) {
        if let Err(_) = stdout().execute(cursor::Show) {
            warn!("Failed to bring back cursor");
        }

        if let Err(_) = stdout().execute(terminal::Clear(terminal::ClearType::All)) {
            warn!("Failed to clear screen on close.");
        }

        if let Err(_) = stdout().execute(terminal::LeaveAlternateScreen) {
            warn!("Failed to return to alt screen.");
        }

        if let Err(_) = terminal::disable_raw_mode() {
            warn!("Failed to disable raw mode on close.");
        }
    }
}

fn main() {
    log4rs::init_file("config/log4rs.yaml", Default::default()).unwrap();
    log_panics::init();

    let (cols, rows) = terminal::size().expect("Failed to get term size.");
    info!("Term size - rows: {}, cols: {}", rows, cols);
    let mut screen = Screen::new(rows, cols);
    let mut state = State::new(cols as usize, rows as usize);
    screen.update(&state).expect("Failed to init screen.");

    let mut last_step = Instant::now();

    loop {
        let ev_ready = event::poll(Duration::from_secs(0)).expect("Failed to pull for event");
        if ev_ready {
            let ev = event::read().expect("Failed to read event.");
            match ev {
                event::Event::Key(key_event) => {
                    let should_exit = state.handle_key(key_event);

                    if should_exit {
                        break;
                    }
                }
                event::Event::Resize(cols, rows) => {
                    if let Err(err) = screen.resize(rows, cols) {
                        error!("Got error while resizing screen: {err}");
                    }

                    state.resize(rows as usize, cols as usize);
                }
                _ => {}
            };
        }

        if last_step.elapsed().as_millis() >= 500 && state.mode == GameMode::Running {
            state.step();
            last_step = Instant::now();
        }

        screen.update(&state).expect("Failed to update screen");
    }
}
