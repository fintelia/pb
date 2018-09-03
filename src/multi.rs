use pb::ProgressBar;
use std::io::{Result, Stdout, Write};
use std::iter::repeat;
use std::str::from_utf8;
use std::sync::mpsc;
use std::sync::mpsc::{Receiver, Sender};
use tty::move_cursor_up;

macro_rules! repeat {
    ($s: expr, $n: expr) => {{
        &repeat($s).take($n).collect::<String>()
    }};
}

pub struct MultiBar<T: Write> {
    nlines: usize,

    lines: Vec<String>,

    nbars: usize,

    chan: (Sender<WriteMsg>, Receiver<WriteMsg>),

    handle: T,
}

impl MultiBar<Stdout> {
    /// Create a new MultiBar with stdout as a writer.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use std::thread;
    /// use pbr::MultiBar;
    /// use std::time::Duration;
    ///
    /// let mut mb = MultiBar::new();
    /// mb.println("Application header:");
    ///
    /// # let count = 250;
    /// let mut p1 = mb.create_bar(count);
    /// let _ = thread::spawn(move || {
    ///     for _ in 0..count {
    ///         p1.inc();
    ///         thread::sleep(Duration::from_millis(100));
    ///     }
    ///     // notify the multibar that this bar finished.
    ///     p1.finish();
    /// });
    ///
    /// mb.println("add a separator between the two bars");
    ///
    /// let mut p2 = mb.create_bar(count * 2);
    /// let _ = thread::spawn(move || {
    ///     for _ in 0..count * 2 {
    ///         p2.inc();
    ///         thread::sleep(Duration::from_millis(100));
    ///     }
    ///     // notify the multibar that this bar finished.
    ///     p2.finish();
    /// });
    ///
    /// // start listen to all bars changes.
    /// // this is a blocking operation, until all bars will finish.
    /// // to ignore blocking, you can run it in a different thread.
    /// mb.listen();
    /// ```
    pub fn new() -> MultiBar<Stdout> {
        MultiBar::on(::std::io::stdout())
    }
}

impl<T: Write> MultiBar<T> {
    /// Create a new MultiBar with an arbitrary writer.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use pbr::MultiBar;
    /// use std::io::stderr;
    ///
    /// let mut mb = MultiBar::on(stderr());
    /// // ...
    /// // see full example in `MultiBar::new`
    /// // ...
    /// ```
    pub fn on(handle: T) -> MultiBar<T> {
        MultiBar {
            nlines: 0,
            nbars: 0,
            lines: Vec::new(),
            chan: mpsc::channel(),
            handle: handle,
        }
    }

    /// println used to add text lines between the bars.
    /// for example: you could add a header to your application,
    /// or text separators between bars.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use pbr::MultiBar;
    ///
    /// let mut mb = MultiBar::new();
    /// mb.println("Application header:");
    ///
    /// # let count = 250;
    /// let mut p1 = mb.create_bar(count);
    /// // ...
    ///
    /// mb.println("Text line between bar1 and bar2");
    ///
    /// let mut p2 = mb.create_bar(count);
    /// // ...
    ///
    /// mb.println("Text line between bar2 and bar3");
    ///
    /// // ...
    /// // ...
    /// mb.listen();
    /// ```
    pub fn println(&mut self, s: &str) {
        self.lines.push(s.to_owned());
        self.nlines += 1;
    }

    /// create_bar creates new `ProgressBar` with `Pipe` as the writer.
    ///
    /// The ordering of the method calls is important. it means that in
    /// the first call, you get a progress bar in level 1, in the 2nd call,
    /// you get a progress bar in level 2, and so on.
    ///
    /// ProgressBar that finish its work, must call `finish()` (or `finish_print`)
    /// to notify the `MultiBar` about it.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use pbr::MultiBar;
    ///
    /// let mut mb = MultiBar::new();
    /// # let (count1, count2, count3) = (250, 62500, 15625000);
    ///
    /// // progress bar in level 1
    /// let mut p1 = mb.create_bar(count1);
    /// // ...
    ///
    /// // progress bar in level 2
    /// let mut p2 = mb.create_bar(count2);
    /// // ...
    ///
    /// // progress bar in level 3
    /// let mut p3 = mb.create_bar(count3);
    ///
    /// // ...
    /// mb.listen();
    /// ```
    pub fn create_bar(&mut self, total: u64) -> ProgressBar<Pipe> {
        self.println("");
        self.nbars += 1;
        let mut p = ProgressBar::on(
            Pipe {
                level: self.nlines - 1,
                chan: self.chan.0.clone(),
            },
            total,
        );
        p.is_multibar = true;
        p.add(0);
        p
    }

    /// listen start listen to all bars changes.
    ///
    /// `ProgressBar` that finish its work, must call `finish()` (or `finish_print`)
    /// to notify the `MultiBar` about it.
    ///
    /// This is a blocking operation and blocks until all bars will
    /// finish.
    /// To ignore blocking, you can run it in a different thread.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use std::thread;
    /// use pbr::MultiBar;
    ///
    /// let mut mb = MultiBar::new();
    ///
    /// // ...
    /// // create some bars here
    /// // ...
    ///
    /// thread::spawn(move || {
    ///     mb.listen();
    ///     println!("all bars done!");
    /// });
    ///
    /// // ...
    /// ```
    pub fn listen(mut self) {
        drop(self.chan.0);

        let mut nlines = 0;
        let mut nblank_lines = 0;
        let mut max_width = 0;
        while let Ok(msg) = self.chan.1.recv() {
            self.lines[msg.level] = msg.string;

            // and draw
            let mut out = String::new();
            if nlines + nblank_lines > 0 {
                out += &move_cursor_up(nlines + nblank_lines);
            }

            let mut new_nlines = 0;
            for l in self.lines.iter() {
                if l.len() > 0 {
                    max_width = max_width.max(l.len());
                    out.push_str(&format!("\r{}\n", l));
                    new_nlines += 1;
                }
            }

            nblank_lines = nlines - new_nlines.min(nlines);
            nlines = new_nlines;

            for _ in 0..nblank_lines {
                out.push_str(&format!("\r\r{}\n", repeat!(" ", max_width - 1)));
            }

            printfl!(self.handle, "{}", out);
        }

        if nlines > 0 {
            let mut out = String::new();
            out += &move_cursor_up(nlines);
            for _ in 0..nlines {
                out.push_str(&format!("\r{}\n", repeat!(" ", max_width - 1)));
            }
            printfl!(self.handle, "{}", out);
            printfl!(self.handle, "{}", move_cursor_up(nlines));
        }
    }
}

pub struct Pipe {
    level: usize,
    chan: Sender<WriteMsg>,
}

impl Write for Pipe {
    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        let s = from_utf8(buf).unwrap().to_owned();
        self.chan
            .send(WriteMsg {
                level: self.level,
                string: s,
            }).unwrap();
        Ok(1)
    }

    fn flush(&mut self) -> Result<()> {
        Ok(())
    }
}

// WriteMsg is the message format used to communicate
// between MultiBar and its bars
struct WriteMsg {
    level: usize,
    string: String,
}
