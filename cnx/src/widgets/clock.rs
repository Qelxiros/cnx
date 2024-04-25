use crate::clock_stream::ClockStream;
use anyhow::Result;
use chrono::{Local, Timelike};
use futures::StreamExt;
use std::marker::PhantomData;
use tokio::time::Duration;

use crate::text::{Attributes, Text};
use crate::widgets::{Widget, WidgetStream};

pub struct Days;
pub struct Hours;
pub struct Minutes;
pub struct Seconds;
pub trait Precision {}
impl Precision for Days {}
impl Precision for Hours {}
impl Precision for Minutes {}
impl Precision for Seconds {}

/// Shows the current time and date.
///
/// This widget shows the current time and date, in the form `%Y-%m-%d %a %I:%M
/// %p`, e.g. `2017-09-01 Fri 12:51 PM`.
pub struct Clock<P: Precision> {
    attr: Attributes,
    format_str: Option<String>,
    phantom: PhantomData<P>,
}

impl<P: Precision> Clock<P> {
    // Creates a new Clock widget.
    pub fn new(attr: Attributes, format_str: Option<String>) -> Clock<P> {
        Clock::<P> {
            attr,
            format_str,
            phantom: PhantomData::<P>,
        }
    }

    fn tick(&self) -> Vec<Text> {
        let now = chrono::Local::now();
        let format_time: String = self
            .format_str
            .clone()
            .map_or("%Y-%m-%d %a %I:%M %p".to_string(), |item| item);
        let text = now.format(&format_time).to_string();
        let texts = vec![Text {
            attr: self.attr.clone(),
            text,
            stretch: false,
            markup: true,
        }];
        texts
    }
}

impl Widget for Clock<Days> {
    fn into_stream(self: Box<Self>) -> Result<WidgetStream> {
        let stream = ClockStream::new(|| {
            let now = Local::now();
            Duration::from_secs(60 * (60 * (24 - now.hour()) + 60 - now.minute()) as u64)
        })
        .map(move |_| Ok(self.tick()));
        Ok(Box::pin(stream))
    }
}

impl Widget for Clock<Hours> {
    fn into_stream(self: Box<Self>) -> Result<WidgetStream> {
        let stream = ClockStream::new(|| {
            let now = Local::now();
            Duration::from_secs(60 * (60 - now.minute()) as u64)
        })
        .map(move |_| Ok(self.tick()));
        Ok(Box::pin(stream))
    }
}

impl Widget for Clock<Minutes> {
    fn into_stream(self: Box<Self>) -> Result<WidgetStream> {
        let stream = ClockStream::new(|| Duration::from_secs((60 - Local::now().second()) as u64))
            .map(move |_| Ok(self.tick()));
        Ok(Box::pin(stream))
    }
}

impl Widget for Clock<Seconds> {
    fn into_stream(self: Box<Self>) -> Result<WidgetStream> {
        let stream = ClockStream::new(|| {
            Duration::from_nanos(1_000_000_000 - (Local::now().nanosecond() % 1_000_000_000) as u64)
        })
        .map(move |_| Ok(self.tick()));
        Ok(Box::pin(stream))
    }
}
