use serde_json::Value;
use std::io;

const MAX_FRAME: usize = 32 * 1024 * 1024;
const FILTER_AT: usize = 1024 * 1024;
const MAX_FILTER_INPUT: usize = 128 * 1024 * 1024;
const IMAGE_AT: usize = 64 * 1024;
const IMAGE_PREFIXES: [&[u8]; 7] = [
    b"iVBORw0KGgo",
    b"/9j/",
    b"R0lGOD",
    b"data:image/png;base64,",
    b"data:image/jpeg;base64,",
    b"data:image/webp;base64,",
    b"data:image/gif;base64,",
];

#[derive(Default)]
pub(super) struct Frames {
    pending: Vec<u8>,
    large: Option<LargeFrame>,
}

pub(super) enum Frame {
    Message {
        message: Value,
        removed_image_bytes: usize,
    },
    Dropped {
        bytes: usize,
        reason: &'static str,
    },
}

impl Frames {
    pub fn push(&mut self, bytes: &[u8]) {
        self.pending.extend_from_slice(bytes);
    }

    pub fn next(&mut self) -> io::Result<Option<Frame>> {
        if self.large.is_none() {
            let Some(header) = self.pending.first_chunk::<4>() else {
                return Ok(None);
            };
            let bytes = u32::from_le_bytes(*header) as usize;
            if bytes == 0 {
                return Err(io::Error::new(io::ErrorKind::InvalidData, "empty frame"));
            }
            if bytes < FILTER_AT {
                let Some(body) = self.pending.get(4..4 + bytes) else {
                    return Ok(None);
                };
                let message = serde_json::from_slice(body)?;
                self.pending.drain(..4 + bytes);
                return Ok(Some(Frame::Message {
                    message,
                    removed_image_bytes: 0,
                }));
            }
            self.pending.drain(..4);
            self.large = Some(LargeFrame::new(bytes));
        }

        let large = self.large.as_mut().unwrap();
        let take = large.remaining.min(self.pending.len());
        large.push(&self.pending[..take]);
        self.pending.drain(..take);
        Ok((large.remaining == 0).then(|| self.large.take().unwrap().finish()))
    }
}

struct LargeFrame {
    bytes: usize,
    remaining: usize,
    filter: Result<ImageFilter, &'static str>,
}

impl LargeFrame {
    fn new(bytes: usize) -> Self {
        Self {
            bytes,
            remaining: bytes,
            filter: if bytes <= MAX_FILTER_INPUT {
                Ok(ImageFilter::default())
            } else {
                Err("input-limit")
            },
        }
    }

    fn push(&mut self, bytes: &[u8]) {
        self.remaining -= bytes.len();
        // The next frame starts at the declared boundary, even if this body was dropped.
        if let Ok(filter) = &mut self.filter {
            if let Err(reason) = filter.push(bytes) {
                self.filter = Err(reason);
            }
        }
    }

    fn finish(self) -> Frame {
        let decoded = self.filter.and_then(|filter| {
            if !matches!(filter.state, StringState::Outside) {
                return Err("invalid-json");
            }
            let message = serde_json::from_slice(&filter.json).map_err(|_| "invalid-json")?;
            Ok(Frame::Message {
                message,
                removed_image_bytes: filter.removed_image_bytes,
            })
        });
        decoded.unwrap_or_else(|reason| Frame::Dropped {
            bytes: self.bytes,
            reason,
        })
    }
}

#[derive(Default)]
enum StringState {
    #[default]
    Outside,
    Json {
        start: usize,
        escaped: bool,
        object_value: bool,
    },
    RemovingImage,
}

#[derive(Default)]
struct ImageFilter {
    json: Vec<u8>,
    state: StringState,
    removed_image_bytes: usize,
    after_colon: bool,
}

impl ImageFilter {
    fn push(&mut self, bytes: &[u8]) -> Result<(), &'static str> {
        for &byte in bytes {
            if matches!(self.state, StringState::RemovingImage) {
                if is_base64_byte(byte) {
                    self.removed_image_bytes += 1;
                    continue;
                }
                if byte != b'"' {
                    return Err("invalid-image-string");
                }
                self.state = StringState::Outside;
            } else {
                match &mut self.state {
                    StringState::Outside if byte == b'"' => {
                        self.state = StringState::Json {
                            start: self.json.len() + 1,
                            escaped: false,
                            object_value: std::mem::take(&mut self.after_colon),
                        };
                    }
                    StringState::Outside if !byte.is_ascii_whitespace() => {
                        self.after_colon = byte == b':';
                    }
                    StringState::Json {
                        start,
                        escaped,
                        object_value,
                    } => {
                        if *escaped {
                            *escaped = false;
                        } else if byte == b'\\' {
                            *escaped = true;
                        } else if byte == b'"' {
                            self.state = StringState::Outside;
                        } else if *object_value
                            && self.json.len() - *start == IMAGE_AT
                            && is_base64_byte(byte)
                        {
                            let text = &self.json[*start..];
                            if IMAGE_PREFIXES.iter().any(|prefix| {
                                text.strip_prefix(*prefix)
                                    .is_some_and(|tail| tail.iter().copied().all(is_base64_byte))
                            }) {
                                // Keep the value envelope and item positions for subsequent patches.
                                self.removed_image_bytes += text.len() + 1;
                                self.json.truncate(*start);
                                self.state = StringState::RemovingImage;
                                continue;
                            }
                        }
                    }
                    StringState::Outside | StringState::RemovingImage => {}
                }
            }
            if self.json.len() == MAX_FRAME {
                return Err("filtered-size-limit");
            }
            self.json.push(byte);
        }
        Ok(())
    }
}

fn is_base64_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'+' | b'/' | b'=')
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn filters_a_53_mib_image_before_parsing_and_preserves_the_next_frame() {
        let prefix = br#"{"requests":[],"items":[{"type":"imageGeneration","result":"iVBORw0KGgo"#;
        let tail = br#""},{"type":"agentMessage","text":"work \"continues\""}]}"#;
        let payload = 53 * 1024 * 1024;
        let mut frames = Frames::default();
        let header = ((prefix.len() + payload + tail.len()) as u32).to_le_bytes();
        frames.push(&header[..2]);
        assert!(frames.next().unwrap().is_none());
        frames.push(&header[2..]);
        frames.push(prefix);
        assert!(frames.next().unwrap().is_none());
        let chunk = [b'A'; 16 * 1024];
        for _ in 0..payload / chunk.len() {
            frames.push(&chunk);
            assert!(frames.next().unwrap().is_none());
        }
        let buffered = frames
            .large
            .as_ref()
            .unwrap()
            .filter
            .as_ref()
            .unwrap()
            .json
            .len();
        assert!(buffered <= prefix.len());
        frames.push(tail);
        frames.push(&2u32.to_le_bytes());
        frames.push(b"{}");
        let Some(Frame::Message {
            message,
            removed_image_bytes,
        }) = frames.next().unwrap()
        else {
            panic!("expected an image-free result");
        };
        assert_eq!(removed_image_bytes, payload + b"iVBORw0KGgo".len());
        assert_eq!(message["items"][0]["result"], "");
        assert_eq!(message["items"][1]["text"], "work \"continues\"");
        assert!(
            matches!(frames.next().unwrap(), Some(Frame::Message { message, removed_image_bytes: 0 }) if message == json!({}))
        );
        assert!(frames.next().unwrap().is_none());
    }

    #[test]
    fn drains_unfilterable_frames_without_losing_the_next_frame() {
        let bytes = MAX_FRAME + 512;
        let mut frames = Frames::default();
        frames.push(&(bytes as u32).to_le_bytes());
        let chunk = [b' '; 16 * 1024];
        for _ in 0..bytes / chunk.len() {
            frames.push(&chunk);
            assert!(frames.next().unwrap().is_none());
        }
        frames.push(&chunk[..bytes % chunk.len()]);
        frames.push(&2u32.to_le_bytes());
        frames.push(b"{}");
        assert!(
            matches!(frames.next().unwrap(), Some(Frame::Dropped { bytes: n, reason: "filtered-size-limit" }) if n == bytes)
        );
        assert!(
            matches!(frames.next().unwrap(), Some(Frame::Message { message, .. }) if message == json!({}))
        );
    }

    #[test]
    fn rejects_a_malformed_image_and_keeps_escaped_non_image_text() {
        let mut filter = ImageFilter::default();
        filter
            .push(br#"{"result":"data:image/jpeg;base64,"#)
            .unwrap();
        filter.push(&vec![b'A'; IMAGE_AT]).unwrap();
        assert_eq!(filter.push(b"!"), Err("invalid-image-string"));

        let mut filter = ImageFilter::default();
        filter.push(br#"{"text":" escaped \"quote\" \\"#).unwrap();
        filter.push(br#"","child":{"value":"ok"}}"#).unwrap();
        assert_eq!(filter.removed_image_bytes, 0);
        let value: Value = serde_json::from_slice(&filter.json).unwrap();
        assert_eq!(value["child"]["value"], "ok");
    }
}
