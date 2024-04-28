use futures::stream;

use crate::text::{Attributes, Text};

use super::Widget;

pub struct Separator {
    attr: Attributes,
    text: String,
}

impl Separator {
    pub fn new(attr: Attributes, text: String) -> Self {
        Self { attr, text }
    }
}

impl Widget for Separator {
    fn into_stream(self: Box<Self>) -> anyhow::Result<super::WidgetStream> {
        Ok(Box::pin(stream::once(async {
            Ok(vec![Text {
                attr: self.attr,
                text: self.text,
                stretch: false,
                markup: true,
            }])
        })))
    }
}
