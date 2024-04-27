use std::fs;

use cnx::{
    text::{Attributes, Text},
    widgets::Widget,
};
use inotify::WatchMask;
use tokio_stream::StreamExt;

pub struct Inotify {
    attr: Attributes,
    filepath: String,
    flags: WatchMask,
}

impl Inotify {
    pub fn new(attr: Attributes, filepath: String, flags: WatchMask) -> Self {
        Self {
            attr,
            filepath,
            flags,
        }
    }

    fn tick(&self) -> anyhow::Result<Vec<Text>> {
        let contents = fs::read_to_string(self.filepath.clone())?;
        let text = contents
            .strip_suffix("\n")
            .unwrap_or(contents.as_str())
            .to_string();
        let texts = vec![Text {
            attr: self.attr.clone(),
            text,
            stretch: false,
            markup: true,
        }];
        Ok(texts)
    }
}

impl Widget for Inotify {
    fn into_stream(self: Box<Self>) -> anyhow::Result<cnx::widgets::WidgetStream> {
        let mut inotify = inotify::Inotify::init()?;

        inotify.watches().add(self.filepath.clone(), self.flags)?;

        let buffer = [0; 32];
        let stream = tokio_stream::once(self.tick())
            .chain(inotify.into_event_stream(buffer)?.map(move |_| self.tick()));
        Ok(Box::pin(stream))
    }
}
