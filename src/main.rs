use crossterm::event;
use crossterm::{ExecutableCommand, QueueableCommand, cursor, style, terminal};
use log::{error, info, warn};
use log4rs;
use std::io::{Stdout, Write, stdout};

struct State {}

struct ScreenBuf {
    rows: u16,
    cols: u16,
    back: Vec<Vec<char>>,
    front: Vec<Vec<char>>,
}

impl ScreenBuf {
    pub fn new(rows: u16, cols: u16) -> ScreenBuf {
        ScreenBuf {
            rows: rows,
            cols: cols,
            back: vec![vec![' '; cols as usize]; rows as usize],
            front: vec![vec![' '; cols as usize]; rows as usize],
        }
    }

    pub fn clear(&mut self) {
        for row in &mut self.back {
            for c in row {
                *c = ' ';
            }
        }
    }

    pub fn clear_row(&mut self, row: u16) {
        for c in &mut self.back[row as usize] {
            *c = ' '
        }
    }

    pub fn write(&mut self, row: u16, col: u16, val: char) {
        self.back[row as usize][col as usize] = val;
    }

    pub fn flush(&mut self, out: &mut Stdout) -> std::io::Result<()> {
        // TODO: make sure this traversal is cache friendly
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

pub struct Screen {
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
        self.screen_buf.flush(&mut out)?;

        out.flush()
    }
}

impl Drop for Screen {
    fn drop(&mut self) {
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
    let state = State {};
    screen.update(&state).expect("Failed to init screen.");

    loop {
        let ev = event::read().expect("Failed to read event.");
        match ev {
            event::Event::Key(key_event) => {
                if key_event.code.is_char('q') {
                    break;
                }
            }
            event::Event::Resize(cols, rows) => {
                if let Err(err) = screen.resize(rows, cols) {
                    error!("Got error while resizing screen: {err}");
                }

                if let Err(err) = screen.update(&state) {
                    error!("Got error while updating screen: {err}");
                }
            }
            _ => {}
        };
    }
}
