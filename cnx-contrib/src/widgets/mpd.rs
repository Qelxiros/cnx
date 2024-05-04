use std::{
    pin::Pin,
    sync::{Arc, Mutex},
    task::Poll,
    time::{Duration, Instant},
};

use anyhow::{anyhow, Result};
use cnx::{
    text::{Attributes, Color, Text},
    widgets::Widget,
};
use futures::stream;
use mpd::{Client, Idle, Subsystem};
use tokio::task::JoinHandle;
use tokio::{task, time, time::Interval};
use tokio_stream::Stream;
use tokio_stream::{StreamExt, StreamMap};

/// Represents MPD widget used to show information about currently playing music
pub struct Mpd {
    pub attr: Attributes,
    conn: Arc<Mutex<Client>>,
    noidle_conn: Arc<Mutex<Client>>,
    highlight_conn: Option<Arc<Mutex<Client>>>,
    pub subsystems: Vec<Subsystem>,
    pub render: fn(Arc<Mutex<Client>>) -> Option<String>,
    pub progress_bar: bool,
    last_sync: Instant,
    last_string: Arc<Mutex<String>>,
    song_length: Option<Duration>,
    song_elapsed: Option<Duration>,
}

impl Mpd {
    /// Creates a new [`Mpd`] widget.
    ///
    /// * `attr` - Represents [`Attributes`] which controls properties like
    /// `Font`, foreground and background color, etc.
    ///
    /// * `socket` - Describes how to connect to the running MPD instance.
    /// Defaults to `127.0.0.1:6600` when [`None`].
    ///
    /// * `subsystems` - Represents which of MPD's subsystems should cause an
    /// interrupt. If you use a subsystem in `render`, you should probably list
    /// it here.
    ///
    /// * `render` - Used to format information before it's displayed. Defaults
    /// to `artist - title` when [`None`].
    ///
    /// * `progress_bar` - Whether or not to show a progress bar by highlighting
    /// part of the text
    ///
    /// # Examples
    ///
    /// ```
    /// use anyhow::Result;
    /// use cnx::text::*;
    /// use cnx::*;
    /// use cnx_contrib::widgets::mpd::*;
    ///
    /// fn main() -> Result<()> {
    ///     let attr = Attributes {
    ///         font: Font::new("Fira Code 21"),
    ///         fg_color: Color::white(),
    ///         bg_color: None,
    ///         padding: Padding::new(0.0, 0.0, 0.0, 0.0),
    ///     };
    ///
    ///     let mut cnx = Cnx::new(Position::Top);
    ///     let m = Mpd::new(
    ///         attr.clone(),
    ///         None,
    ///         Vec::new(),
    ///         Some(|conn| conn.lock().unwrap().currentsong().ok()??.title),
    ///         true,
    ///     )
    ///     .unwrap();
    ///     cnx.add_widget(m);
    ///     cnx.run()?;
    ///
    ///     Ok(())
    /// }
    /// ```
    pub fn new(
        attr: Attributes,
        socket: Option<String>,
        subsystems: Vec<Subsystem>,
        render: Option<fn(Arc<Mutex<Client>>) -> Option<String>>,
        progress_bar: bool,
    ) -> Result<Self> {
        let socket = socket.unwrap_or("127.0.0.1:6600".into());
        Ok(Self {
            attr,
            conn: Arc::new(Mutex::new(Client::connect(socket.clone())?)),
            noidle_conn: Arc::new(Mutex::new(Client::connect(socket.clone())?)),
            highlight_conn: Some(
                progress_bar
                    .then(|| {
                        Ok::<Arc<Mutex<Client>>, anyhow::Error>(Arc::new(Mutex::new(
                            Client::connect(socket.clone())?,
                        )))
                    })
                    .ok_or(anyhow!("Failed to establish MPD connection"))??,
            ),
            subsystems,
            render: render.unwrap_or(|conn| {
                let currentsong = conn.lock().unwrap().currentsong().ok()??;
                Some(format!(
                    "{} {}",
                    currentsong.artist.unwrap_or("Unknown".into()),
                    currentsong.title.unwrap_or("Unknown".into())
                ))
            }),
            progress_bar,
            last_sync: Instant::now(),
            last_string: Arc::new(Mutex::new(String::new())),
            song_length: None,
            song_elapsed: None,
        })
    }

    fn tick(&mut self) -> Result<Vec<Text>> {
        let conn = self.noidle_conn.clone();
        self.last_sync = Instant::now();
        self.song_elapsed = conn.lock().unwrap().status()?.elapsed;
        self.song_length = conn.lock().unwrap().status()?.duration;
        let text = (self.render)(self.noidle_conn.clone()).unwrap_or(String::new());
        let length = text.chars().count();
        *self.last_string.lock().unwrap() = text.clone();
        if self.progress_bar && self.song_elapsed.is_some() && self.song_length.is_some() {
            let char_index = ((self.song_elapsed.unwrap() + (Instant::now() - self.last_sync))
                .as_secs_f64()
                / self.song_length.unwrap().as_secs_f64()
                * length as f64)
                .round() as usize;
            let mut chars = text.chars();
            Ok(vec![
                Text {
                    attr: self
                        .attr
                        .clone()
                        .strip_right_padding()
                        .with_bg(Some(Color::red())),
                    text: chars.by_ref().take(char_index).collect(),
                    stretch: false,
                    markup: false,
                },
                Text {
                    attr: self.attr.clone().strip_left_padding(),
                    text: chars.collect(),
                    stretch: false,
                    markup: false,
                },
            ])
        } else {
            Ok(vec![Text {
                attr: self.attr.clone(),
                text,
                stretch: false,
                markup: false,
            }])
        }
    }
}

impl Widget for Mpd {
    fn into_stream(mut self: Box<Self>) -> Result<cnx::widgets::WidgetStream> {
        let _ = self.tick();
        let mut map = StreamMap::<usize, Pin<Box<dyn Stream<Item = Result<()>>>>>::new();
        map.insert(
            0,
            Box::pin(stream::once(async { Ok(()) }).chain(MpdStream {
                conn: self.conn.clone(),
                handle: None,
                subsystems: self.subsystems.clone(),
            })),
        );
        map.insert(
            1,
            Box::pin(HighlightStream {
                last_string: self.last_string.clone(),
                interval: time::interval(Duration::from_secs(10)),
                song_length: None,
                song_elapsed: None,
                conn: self.highlight_conn.clone().unwrap(),
                noidle_conn: self.noidle_conn.clone(),
                handle: None,
                stale: Arc::new(Mutex::new(true)),
            }),
        );
        Ok(Box::pin(map.map(move |_| self.tick())))
    }
}

struct HighlightStream {
    last_string: Arc<Mutex<String>>,
    interval: Interval,
    song_length: Option<Duration>,
    song_elapsed: Option<Duration>,
    conn: Arc<Mutex<Client>>,
    noidle_conn: Arc<Mutex<Client>>,
    handle: Option<JoinHandle<Result<()>>>,
    stale: Arc<Mutex<bool>>,
}

impl Stream for HighlightStream {
    type Item = Result<()>;

    fn poll_next(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> Poll<Option<Self::Item>> {
        if self.handle.is_none() {
            let waker = cx.waker().clone();
            let conn = self.conn.clone();
            let stale = self.stale.clone();
            self.handle = Some(task::spawn_blocking(move || {
                let _ = conn.lock().unwrap().wait(&[Subsystem::Player]);
                if let Ok(mut stale) = stale.lock() {
                    *stale = true;
                }
                waker.wake();
                Ok(())
            }))
        }
        if *self.stale.lock().unwrap() {
            *self.stale.lock().unwrap() = false;
            let conn = self.noidle_conn.clone();
            let mut conn = conn.lock().unwrap();
            let status = conn.status()?;
            self.song_length = status.duration;
            self.song_elapsed = status.elapsed;
            if self.song_length.is_some() && self.song_elapsed.is_some() {
                let string_length = self
                    .last_string
                    .lock()
                    .unwrap()
                    .chars()
                    .count()
                    .try_into()?;
                if string_length > 0 {
                    let interval = time::interval(self.song_length.unwrap() / string_length);
                    self.interval = interval;
                }
            } else {
                // TODO: pause time when song is paused
            }
        }
        self.interval.poll_tick(cx).map(|_| Some(Ok(())))
    }
}

struct MpdStream {
    conn: Arc<Mutex<Client>>,
    handle: Option<JoinHandle<Result<()>>>,
    subsystems: Vec<Subsystem>,
}

impl Stream for MpdStream {
    type Item = Result<()>;

    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> Poll<Option<Self::Item>> {
        if let Some(handle) = &self.handle {
            if handle.is_finished() {
                self.handle = None;
                Poll::Ready(Some(Ok(())))
            } else {
                Poll::Pending
            }
        } else {
            let conn = self.conn.clone();
            let subsystems = self.subsystems.clone();
            let waker = cx.waker().clone();
            self.handle = Some(task::spawn_blocking(move || {
                let _ = conn.lock().unwrap().wait(subsystems.as_slice());
                waker.wake();
                Ok(())
            }));
            Poll::Pending
        }
    }
}
