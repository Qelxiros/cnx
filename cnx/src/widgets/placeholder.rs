use futures::stream;

use crate::text::Text;

use super::Widget;

pub struct Placeholder {
    texts: Vec<Text>,
}

impl Placeholder {
    pub fn new(texts: Vec<Text>) -> Self {
        Placeholder { texts }
    }
}

impl Widget for Placeholder {
    fn into_stream(self: Box<Self>) -> anyhow::Result<super::WidgetStream> {
        Ok(Box::pin(stream::once(async { Ok(self.texts) })))
    }
}
