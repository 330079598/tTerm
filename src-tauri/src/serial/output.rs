use crate::terminal::encoding::{OutputDecoder, TerminalEncoding};
use crate::terminal::reader::{FilteredOutput, OutputFilter};

/// Turns what a serial device sends into what its tab shows: text in the
/// device's charset decoded to UTF-8.
pub(super) struct SerialOutput {
    decoder: Option<OutputDecoder>,
}

impl SerialOutput {
    /// The filter a port needs, or `None` when its output shows as it comes.
    pub(super) fn for_encoding(encoding: TerminalEncoding) -> Option<Self> {
        let decoder = encoding.output_decoder()?;
        Some(Self {
            decoder: Some(decoder),
        })
    }
}

impl OutputFilter for SerialOutput {
    fn filter(&mut self, output: Vec<u8>) -> FilteredOutput {
        let display = match self.decoder.as_mut() {
            Some(decoder) => decoder.decode(&output),
            None => output,
        };
        FilteredOutput { display, log: None }
    }
}
