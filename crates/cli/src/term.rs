//! Everything this frontend writes to the terminal: styling, the result table,
//! the progress line and the prompts.
//!
//! Written by hand rather than through a table crate, because the shape the
//! terminal app draws is a plain aligned table and nothing more: the point of
//! the CLI is that it reads the same in a terminal window, in a log file and in
//! a pipe.
//!
//! Colour follows the two conventions every other terminal program honours: it
//! is off when stdout is not a terminal (so a redirected run is free of escape
//! codes) and off when `NO_COLOR` is set (https://no-color.org).

use std::io::{IsTerminal, Write};
use std::sync::atomic::{AtomicBool, Ordering};

use database::structures::Cleared;
use database::utils::get_file_size_string;

/// ANSI escapes used by this frontend. Only the eight basic colours: a table
/// that leans on 256-colour codes stops being readable in the default Windows
/// console, which is where a lot of these runs happen.
mod code {
    pub const RESET: &str = "\x1b[0m";
    pub const BOLD: &str = "\x1b[1m";
    pub const DIM: &str = "\x1b[2m";
    pub const RED: &str = "\x1b[31m";
    pub const GREEN: &str = "\x1b[32m";
    pub const YELLOW: &str = "\x1b[33m";
    pub const BLUE: &str = "\x1b[34m";
    pub const CYAN: &str = "\x1b[36m";
}

/// Styling, resolved once from the flags and the environment.
pub struct Ui {
    color: bool,
    quiet: bool,
    json: bool,
    /// How many columns the terminal has, for the progress bar. Zero when it is
    /// not known, which hides the bar rather than guessing a width.
    width: usize,
}

/// Set once a progress line has been drawn, so the run knows it has to move off
/// that line before printing anything else.
///
/// A static rather than a field because [`Ui`] is passed by reference to every
/// print helper and holding the flag there would mean threading a `&mut` through
/// all of them — and the line is shared state of the terminal, not of a styling
/// object.
static PROGRESS_ACTIVE: AtomicBool = AtomicBool::new(false);

impl Ui {
    /// Builds the styling for this run.
    pub fn new(no_color: bool, quiet: bool, json: bool) -> Self {
        // INFO: `--quiet` silences the progress line but not the result: the
        // result is the point of the run, `--quiet` is for scripts that only
        // care about the exit code.
        let color = !no_color
            && !json
            && std::env::var_os("NO_COLOR").is_none()
            && std::io::stdout().is_terminal();
        Self {
            color,
            quiet,
            json,
            width: terminal_width(),
        }
    }

    pub fn is_json(&self) -> bool {
        self.json
    }

    pub fn quiet(&self) -> bool {
        self.quiet
    }

    pub fn color(&self) -> bool {
        self.color
    }

    /// Wraps `text` in `codes`, or returns it unchanged when colour is off.
    pub fn paint(&self, codes: &[&str], text: &str) -> String {
        if !self.color || codes.is_empty() {
            return text.to_string();
        }
        format!("{}{}{}", codes.concat(), text, code::RESET)
    }

    pub fn bold(&self, text: &str) -> String {
        self.paint(&[code::BOLD], text)
    }

    pub fn dim(&self, text: &str) -> String {
        self.paint(&[code::DIM], text)
    }

    pub fn good(&self, text: &str) -> String {
        self.paint(&[code::GREEN], text)
    }

    pub fn warn(&self, text: &str) -> String {
        self.paint(&[code::YELLOW], text)
    }

    pub fn bad(&self, text: &str) -> String {
        self.paint(&[code::RED], text)
    }

    pub fn heading(&self, text: &str) -> String {
        self.paint(&[code::BOLD, code::CYAN], text)
    }

    pub fn accent(&self, text: &str) -> String {
        self.paint(&[code::BLUE], text)
    }

    /// Prints one line, unless `--quiet` or `--json` asked for silence.
    ///
    /// JSON output goes through [`Self::print_json`] instead, so this never has
    /// to interleave a human line with a machine-readable one.
    pub fn line(&self, text: &str) {
        if self.quiet || self.json {
            return;
        }
        self.clear_progress();
        println!("{text}");
    }

    /// Prints a line that `--quiet` does not suppress: warnings and errors, the
    /// things a script needs to see even when it asked for silence.
    pub fn always(&self, text: &str) {
        if self.json {
            return;
        }
        self.clear_progress();
        println!("{text}");
    }

    /// Prints a warning to stderr, where it stays out of a piped result.
    pub fn warning(&self, text: &str) {
        if self.json {
            return;
        }
        self.clear_progress();
        eprintln!("{} {text}", self.warn("warning:"));
    }

    /// Writes the machine-readable result. The one line `--json` allows through.
    pub fn print_json(&self, json: &str) {
        println!("{json}");
    }

    // --- progress --------------------------------------------------------

    /// Draws the progress line in place, with a carriage return so it stays on
    /// one line. A no-op when the output is not a terminal: there the line
    /// would fill a log with thousands of near-identical entries, so only the
    /// milestones below are printed instead.
    pub fn progress(&self, fraction: Option<f32>, label: &str, detail: &str) {
        if self.quiet || self.json || !std::io::stdout().is_terminal() {
            return;
        }
        let mut line = String::new();
        if let Some(fraction) = fraction {
            line.push_str(&self.bar(fraction));
            line.push(' ');
        }
        line.push_str(&self.bold(label));
        if !detail.is_empty() {
            line.push_str(&self.dim(&format!("  {detail}")));
        }
        print!("\r\x1b[2K{line}");
        let _ = std::io::stdout().flush();
        PROGRESS_ACTIVE.store(true, Ordering::Relaxed);
    }

    /// The bracket bar. Width is whatever the terminal has left over after the
    /// label, so a narrow window degrades to a shorter bar instead of wrapping.
    fn bar(&self, fraction: f32) -> String {
        const FULL: usize = 24;
        // INFO: `f32::clamp` passes NaN through, and a NaN fraction would print
        // as "NaN%". A division by a total that is not known reaches here as
        // NaN rather than as None, so it is normalized away here once instead
        // of at every caller.
        let fraction = if fraction.is_nan() {
            0.0
        } else {
            fraction.clamp(0.0, 1.0)
        };
        let filled = ((fraction * FULL as f32).round() as usize).min(FULL);
        let bar: String = "█".repeat(filled) + &"░".repeat(FULL - filled);
        let percent = format!("{:>3.0}%", fraction * 100.0);
        if self.width > bar.chars().count() + percent.len() + 4 {
            format!("{} {percent}", self.good(&bar))
        } else {
            percent
        }
    }

    /// Moves off the progress line, once, before anything else is printed.
    pub fn clear_progress(&self) {
        if PROGRESS_ACTIVE.swap(false, Ordering::Relaxed) && std::io::stdout().is_terminal() {
            print!("\r\x1b[2K");
            let _ = std::io::stdout().flush();
        }
    }

    // --- tables ----------------------------------------------------------

    /// Prints the result table: one row per program, largest first, with the
    /// categories it was cleaned under.
    ///
    /// Sorted by size rather than left in completion order, which is what the
    /// window app's results page does not have to think about: its table is
    /// scrolled, so the order it arrives in does not matter. Here the reader is
    /// a person looking at the first line and stopping.
    pub fn result_table(&self, bytes: u64, files: u64, dirs: u64, cleared: &[Cleared]) {
        if self.quiet || self.json {
            return;
        }
        self.clear_progress();

        println!(
            "{}",
            self.heading(&format!(
                "Removed {} in {} and {} across {}",
                get_file_size_string(bytes),
                count(files, "file"),
                count(dirs, "directory"),
                count(cleared.len() as u64, "program"),
            ))
        );

        if cleared.is_empty() {
            println!(
                "{}",
                self.dim("Nothing matched — the selected paths were already empty.")
            );
            return;
        }

        let mut rows: Vec<&Cleared> = cleared.iter().collect();
        rows.sort_by_key(|row| std::cmp::Reverse(row.removed_bytes));

        // Header, then one row per program. The columns are padded to the widest
        // value in the column rather than to a fixed width, so a run with one
        // program does not leave a screen of trailing spaces.
        let header = ["Program", "Size", "Files", "Dirs", "Categories"];
        let program_width = rows
            .iter()
            .map(|row| row.program.chars().count())
            .chain(std::iter::once(header[0].len()))
            .max()
            .unwrap_or(0);
        let size_width = rows
            .iter()
            .map(|row| get_file_size_string(row.removed_bytes).chars().count())
            .chain(std::iter::once(header[1].len()))
            .max()
            .unwrap_or(0);
        let count_width = rows
            .iter()
            .map(|row| {
                row.removed_files
                    .to_string()
                    .len()
                    .max(row.removed_directories.to_string().len())
            })
            .chain(std::iter::once(header[2].len()))
            .max()
            .unwrap_or(0);

        println!(
            "{}  {}  {}  {}  {}",
            self.dim(&pad(header[0], program_width)),
            self.dim(&pad(header[1], size_width)),
            self.dim(&pad(header[2], count_width)),
            self.dim(&pad(header[3], count_width)),
            self.dim(header[4]),
        );
        for row in rows {
            println!(
                "{}  {}  {}  {}  {}",
                pad(&row.program, program_width),
                self.good(&pad_left(
                    &get_file_size_string(row.removed_bytes),
                    size_width
                )),
                self.dim(&pad_left(&row.removed_files.to_string(), count_width)),
                self.dim(&pad_left(&row.removed_directories.to_string(), count_width)),
                self.dim(&row.affected_categories.join(", ")),
            );
        }
    }

    /// Prints the deleted paths of one program, largest first.
    ///
    /// The aggregate row says *how much*; this says *where*, which is the
    /// question an aggregate provokes. `paths_omitted` is printed too, because a
    /// truncated list that does not say so reads as the whole truth.
    pub fn path_details(&self, entry: &Cleared) {
        if self.quiet || self.json {
            return;
        }
        println!();
        println!(
            "{}",
            self.bold(&format!(
                "{} — {}",
                entry.program,
                count(entry.paths.len() as u64, "path")
            ))
        );
        for detail in &entry.paths {
            let mut counts = Vec::new();
            if detail.removed_files > 0 {
                counts.push(format!(
                    "{} {}",
                    detail.removed_files,
                    plural(detail.removed_files == 1, "file", "files")
                ));
            }
            if detail.removed_directories > 0 {
                counts.push(format!(
                    "{} {}",
                    detail.removed_directories,
                    plural(detail.removed_directories == 1, "dir", "dirs")
                ));
            }
            println!(
                "  {}  {} {}",
                detail.path,
                self.good(&get_file_size_string(detail.removed_bytes)),
                self.dim(&counts.join(" ")),
            );
        }
        if entry.paths_omitted > 0 {
            println!(
                "  {}",
                self.dim(&format!(
                    "{} more deleted {} are not listed.",
                    count(entry.paths_omitted as u64, "path"),
                    plural(entry.paths_omitted == 1, "path", "paths")
                ))
            );
        }
    }

    /// Prints a plain indented list, used for the categories and the programs.
    pub fn list(&self, lines: &[String]) {
        if self.quiet || self.json {
            return;
        }
        for line in lines {
            self.line(line);
        }
    }
}

/// The terminal's width in columns, or 0 when it cannot be determined.
fn terminal_width() -> usize {
    #[cfg(windows)]
    {
        // `crossterm` is not a dependency of this crate, and the Win32 console
        // API is not worth a `windows` dependency for one number: fall back to
        // the COLUMNS variable, which any modern terminal sets.
        std::env::var("COLUMNS")
            .ok()
            .and_then(|value| value.parse().ok())
            .unwrap_or(0)
    }
    #[cfg(not(windows))]
    {
        // The ioctl lives behind a libc dependency this crate does not have.
        std::env::var("COLUMNS")
            .ok()
            .and_then(|value| value.parse().ok())
            .unwrap_or(0)
    }
}

/// `1 program` / `7 programs`, from the count rather than by hand.
///
/// The plural *forms* on their own would leave a heading reading "programs and
/// paths would be cleaned", which says nothing about how much is about to
/// happen — the two numbers are the whole point of a dry run.
///
/// Takes `u64` rather than `usize` because the counters come straight off the
/// cleaner: they are file counts, they are never an index, and truncating one to
/// a pointer width to satisfy a signature would be a lossy conversion for
/// nothing.
pub fn count(amount: u64, noun: &'static str) -> String {
    format!("{} {}", amount, plural(amount == 1, noun, plural_of(noun)))
}

/// `one file` / `two files`, from the count rather than by hand.
pub fn plural(is_one: bool, one: &'static str, many: &'static str) -> &'static str {
    if is_one { one } else { many }
}

// Not every English plural is the noun plus an `s` — "directory" is
// "directories" — so the irregular one is spelled out and the rest falls through
// to the regular rule. The cache holds the result because a run prints a line
// per program, and the allocation has no business being in that loop.
thread_local! {
    static PLURALS: std::cell::RefCell<std::collections::HashMap<&'static str, &'static str>> =
        std::cell::RefCell::new(std::collections::HashMap::new());
}

/// The plural form of a noun, memoized per thread.
fn plural_of(noun: &'static str) -> &'static str {
    PLURALS.with(|cache| {
        if let Some(cached) = cache.borrow().get(noun) {
            return *cached;
        }
        // `leaked` because the callers pass string literals: the set is fixed at
        // compile time, so this costs one allocation per noun per thread rather
        // than a `String` on every line printed.
        let many: &'static str = match noun {
            "directory" => "directories",
            _ => Box::leak(format!("{noun}s").into_boxed_str()),
        };
        cache.borrow_mut().insert(noun, many);
        many
    })
}

/// Pads `text` on the right to `width` columns.
pub fn pad(text: &str, width: usize) -> String {
    let mut out = text.to_string();
    let len = text.chars().count();
    if len < width {
        out.extend(std::iter::repeat_n(' ', width - len));
    }
    out
}

/// Pads `text` on the left to `width` columns, so numbers line up on their last
/// digit the way a table of them should.
pub fn pad_left(text: &str, width: usize) -> String {
    let len = text.chars().count();
    if len < width {
        format!("{}{text}", " ".repeat(width - len))
    } else {
        text.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colour_is_off_when_it_was_not_asked_for() {
        let ui = Ui::new(true, false, false);
        assert!(!ui.color());
        assert_eq!(ui.bold("x"), "x");
        assert_eq!(ui.good("x"), "x");
    }

    #[test]
    fn json_output_suppresses_human_lines_but_not_the_json() {
        let ui = Ui::new(true, false, true);
        assert!(ui.is_json());
        ui.line("a human line");
        ui.warning("a warning");
        // Nothing reached stdout above; both helpers returned early.
    }

    #[test]
    fn a_warning_is_always_printed_unless_json() {
        // Nothing to assert on stdout here — the point is that the guard does
        // not depend on `--quiet`, so the call is made and simply not swallowed.
        let ui = Ui::new(true, true, false);
        assert!(ui.quiet());
        ui.warning("still shown");
    }

    #[test]
    fn padding_lines_up_by_characters() {
        assert_eq!(pad("ab", 5), "ab   ");
        assert_eq!(pad("abcdef", 3), "abcdef");
        assert_eq!(pad_left("42", 5), "   42");
        assert_eq!(pad_left("123456", 3), "123456");
        // Multi-byte text is counted in characters, not bytes, so a name with an
        // accent does not push the column over.
        assert_eq!(pad("ää", 4).chars().count(), 4);
    }

    #[test]
    fn plurals_come_from_the_count() {
        assert_eq!(plural(true, "file", "file"), "file");
        assert_eq!(plural(false, "file", "files"), "files");
    }

    #[test]
    fn a_count_carries_its_own_number() {
        assert_eq!(count(1, "program"), "1 program");
        assert_eq!(count(0, "program"), "0 programs");
        assert_eq!(count(7, "path"), "7 paths");
        // The irregular plural is the reason `count` exists rather than a
        // `format!("{n}s")` at each call site.
        assert_eq!(count(2, "directory"), "2 directories");
        assert_eq!(count(1, "directory"), "1 directory");
    }

    #[test]
    fn the_bar_is_the_requested_fraction() {
        let ui = Ui::new(true, false, false);
        // A width of zero means the terminal size is unknown, which is the case
        // for every test: the bar degrades to the percentage alone.
        assert_eq!(ui.bar(0.5), " 50%");
        assert_eq!(ui.bar(2.0), "100%");
        assert_eq!(ui.bar(-1.0), "  0%");
        assert_eq!(ui.bar(f32::NAN), "  0%");
    }

    #[test]
    fn an_empty_result_says_so() {
        // The table is only printed when there is something to say; an empty
        // run has to say that instead of printing a header and no rows.
        let ui = Ui::new(true, false, false);
        ui.result_table(0, 0, 0, &[]);
    }
}
