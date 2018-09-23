use ezgui::{Canvas, GfxCtx, LogScroller, UserInput};
use log;
use log::{LevelFilter, Log, Metadata, Record};
use objects::ROOT_MENU;
use piston::input::Key;
use plugins::Colorizer;
use std::sync::Mutex;

lazy_static! {
    static ref LOGGER: Mutex<LogScroller> = Mutex::new(LogScroller::new_with_capacity(100));
}

static LOG_ADAPTER: LogAdapter = LogAdapter;

pub struct DisplayLogs {
    active: bool,
}

impl DisplayLogs {
    pub fn new() -> DisplayLogs {
        log::set_max_level(LevelFilter::Debug);
        log::set_logger(&LOG_ADAPTER).unwrap();
        DisplayLogs { active: false }
    }

    pub fn event(&mut self, input: &mut UserInput) -> bool {
        if !self.active {
            if input.unimportant_key_pressed(Key::Comma, ROOT_MENU, "show logs") {
                self.active = true;
                return true;
            } else {
                return false;
            }
        }

        if LOGGER.lock().unwrap().event(input) {
            self.active = false;
        }
        self.active
    }

    pub fn draw(&self, g: &mut GfxCtx, canvas: &Canvas) {
        if self.active {
            LOGGER.lock().unwrap().draw(g, canvas);
        }
    }
}

impl Colorizer for DisplayLogs {}

struct LogAdapter;

impl Log for LogAdapter {
    fn enabled(&self, _metadata: &Metadata) -> bool {
        true
    }

    fn log(&self, record: &Record) {
        let line = format!(
            "[{}] [{}] {}",
            record.level(),
            record.target(),
            record.args()
        );
        println!("{}", line);
        // TODO could handle newlines here
        LOGGER.lock().unwrap().add_line(&line);
    }

    fn flush(&self) {}
}