use std::io::{BufRead, Read};

pub const JFP_VERSION: &str = "v5.4.0";

pub const JFP_HEADER: &str = "[JFP_HEADER]";
pub const JFP_END_HEADER: &str = "[END_HEADER]";
pub const JFP_BODY: &str = "[MESSAGE_BODY]";
pub const JFP_END_MESSAGE: &str = "[END_MESSAGE]";

pub const DEFAULT_MAX_FRAME_BYTES: usize = 1024 * 1024;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JfpMessage {
    pub status: String,
    pub intent: Option<String>,
    pub body: String,
}

impl JfpMessage {
    pub fn reply(status: impl Into<String>, body: impl Into<String>) -> Self {
        Self {
            status: status.into(),
            intent: None,
            body: body.into(),
        }
    }

    pub fn request(intent: impl Into<String>, body: impl Into<String>) -> Self {
        Self {
            status: String::new(),
            intent: Some(intent.into()),
            body: body.into(),
        }
    }

    pub fn to_jfp_string(&self) -> String {
        let mut frame = String::with_capacity(128 + self.body.len());

        frame.push_str(JFP_HEADER);
        frame.push('\n');
        frame.push_str(&format!("VERSION: {JFP_VERSION}\n"));

        if !self.status.is_empty() {
            frame.push_str(&format!("STATUS: {}\n", self.status));
        }

        if let Some(intent) = &self.intent {
            frame.push_str(&format!("INTENT: {intent}\n"));
        }

        frame.push_str(JFP_END_HEADER);
        frame.push('\n');
        frame.push_str(JFP_BODY);
        frame.push('\n');
        frame.push_str(&self.body);
        frame.push('\n');
        frame.push_str(JFP_END_MESSAGE);

        frame
    }

    pub fn parse(input: &str) -> Option<Self> {
        let start = input.find(JFP_HEADER)?;
        let mut rest = &input[start + JFP_HEADER.len()..];

        let header_end = rest.find(JFP_END_HEADER)?;
        let mut status = String::new();
        let mut intent = None;
        let mut version = None;

        for line in rest[..header_end].lines() {
            let line = line.trim();

            if let Some(value) = line.strip_prefix("VERSION:") {
                if version.replace(value.trim()).is_some() {
                    return None;
                }
            } else if let Some(value) = line.strip_prefix("STATUS:") {
                status = value.trim().to_owned();
            } else if let Some(value) = line.strip_prefix("INTENT:") {
                intent = Some(value.trim().to_owned());
            }
        }

        if version != Some(JFP_VERSION) || (status.is_empty() && intent.is_none()) {
            return None;
        }

        rest = &rest[header_end + JFP_END_HEADER.len()..];

        let body_start = rest.find(JFP_BODY)?;
        rest = &rest[body_start + JFP_BODY.len()..];

        let body_end = rest.find(JFP_END_MESSAGE)?;
        let body = rest[..body_end].trim_matches('\n').to_owned();

        Some(Self {
            status,
            intent,
            body,
        })
    }
}

pub struct JfpReader<R> {
    reader: R,
    buffer: String,
    collecting: bool,
    max_frame_bytes: usize,
}

impl<R: BufRead> JfpReader<R> {
    pub fn new(reader: R) -> Self {
        Self::with_limit(reader, DEFAULT_MAX_FRAME_BYTES)
    }

    pub fn with_limit(reader: R, max_frame_bytes: usize) -> Self {
        Self {
            reader,
            buffer: String::new(),
            collecting: false,
            max_frame_bytes,
        }
    }

    pub fn is_incomplete(&self) -> bool {
        self.collecting && !self.buffer.is_empty()
    }

    pub fn next_frame(&mut self) -> Option<JfpMessage> {
        let mut line = String::with_capacity(256);
        loop {
            line.clear();
            match self
                .reader
                .by_ref()
                .take((self.max_frame_bytes + 1) as u64)
                .read_line(&mut line)
            {
                Ok(0) | Err(_) => return None,
                Ok(_) => {}
            }

            if line.len() > self.max_frame_bytes {
                self.buffer.clear();
                self.collecting = false;
                // Consume the rest without allocating an unbounded line.
                if !line.ends_with('\n') {
                    loop {
                        let chunk = self.reader.fill_buf().ok()?;
                        if chunk.is_empty() {
                            return None;
                        }
                        let end = chunk.iter().position(|b| *b == b'\n');
                        let count = end.map_or(chunk.len(), |i| i + 1);
                        self.reader.consume(count);
                        if end.is_some() {
                            break;
                        }
                    }
                }
                continue;
            }

            let marker = line.trim();

            if marker == JFP_HEADER {
                self.buffer.clear();
                self.collecting = true;
            }

            if !self.collecting {
                continue;
            }

            self.buffer.push_str(&line);

            if self.buffer.len() > self.max_frame_bytes {
                self.buffer.clear();
                self.collecting = false;
                continue;
            }

            if marker == JFP_END_MESSAGE {
                let parsed = JfpMessage::parse(&self.buffer);
                self.buffer.clear();
                self.collecting = false;

                if let Some(message) = parsed {
                    return Some(message);
                }

                continue;
            }

            if self.buffer.len() > self.max_frame_bytes {
                self.buffer.clear();
                self.collecting = false;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::{JfpMessage, JfpReader};

    #[test]
    fn rejects_incompatible_version_and_recovers_after_oversize() {
        let frame = JfpMessage::reply("OK", "hello").to_jfp_string();
        assert!(JfpMessage::parse(&frame.replace("v5.4.0", "v999")).is_none());
        assert!(JfpMessage::parse(&frame.replace("VERSION: v5.4.0\n", "")).is_none());
        let stream = format!("{}\n{frame}\n", "x".repeat(4096));
        let mut reader = JfpReader::with_limit(Cursor::new(stream), 256);
        assert_eq!(reader.next_frame().unwrap().body, "hello");
    }

    #[test]
    fn rejects_oversized_frame_even_when_terminated() {
        let large = JfpMessage::reply("OK", "a\n".repeat(128)).to_jfp_string();
        let small = JfpMessage::reply("OK", "next").to_jfp_string();
        let mut reader = JfpReader::with_limit(Cursor::new(format!("{large}\n{small}\n")), 160);
        assert_eq!(reader.next_frame().unwrap().body, "next");
    }

    #[test]
    fn reply_round_trip() {
        let message = JfpMessage::reply("OK", "TRYB: OSINT | Rdzen dziala. Tick: 7");

        assert_eq!(JfpMessage::parse(&message.to_jfp_string()), Some(message));
    }

    #[test]
    fn request_round_trip() {
        let message = JfpMessage::request("COMMAND", "192.168.44.9");
        let frame = message.to_jfp_string();

        assert!(frame.contains("INTENT: COMMAND"));
        assert!(!frame.contains("STATUS:"));
        assert_eq!(JfpMessage::parse(&frame), Some(message));
    }

    #[test]
    fn ignores_garbage_around_message() {
        let raw = "some log line\n{\"noise\":true}\n[JFP_HEADER]\nVERSION: v5.4.0\nSTATUS: ERROR\n[END_HEADER]\n[MESSAGE_BODY]\nboom\n[END_MESSAGE]\ntrailing";

        let parsed = JfpMessage::parse(raw).expect("message should parse");

        assert_eq!(parsed.status, "ERROR");
        assert_eq!(parsed.intent, None);
        assert_eq!(parsed.body, "boom");
    }

    #[test]
    fn rejects_frame_without_status_or_intent() {
        let raw = "[JFP_HEADER]\nVERSION: v5.4.0\n[END_HEADER]\n[MESSAGE_BODY]\nx\n[END_MESSAGE]";

        assert_eq!(JfpMessage::parse(raw), None);
    }

    #[test]
    fn reader_skips_garbage_and_reads_both_intents() {
        let stream = concat!(
            "losowy log\n",
            "[JFP_HEADER]\nVERSION: v5.4.0\nSTATUS: OK\n[END_HEADER]\n[MESSAGE_BODY]\nA\n[END_MESSAGE]\n",
            "smieci pomiedzy ramkami\n",
            "[JFP_HEADER]\nVERSION: v5.4.0\n[END_HEADER]\n[MESSAGE_BODY]\nzla ramka\n[END_MESSAGE]\n",
            "[JFP_HEADER]\nVERSION: v5.4.0\nINTENT: TICK\n[END_HEADER]\n[MESSAGE_BODY]\n7\n[END_MESSAGE]\n",
        );

        let mut reader = JfpReader::new(Cursor::new(stream.as_bytes().to_vec()));

        let first = reader.next_frame().expect("reply");
        assert_eq!(first.status, "OK");

        let second = reader.next_frame().expect("request");
        assert_eq!(second.intent.as_deref(), Some("TICK"));
        assert_eq!(second.body, "7");

        assert_eq!(reader.next_frame(), None);
        assert!(!reader.is_incomplete());
    }

    #[test]
    fn reader_reports_truncated_frame() {
        let stream = "[JFP_HEADER]\nVERSION: v5.4.0\nSTATUS: ERROR\n[MESSAGE_BODY]\nPANIC";
        let mut reader = JfpReader::new(Cursor::new(stream.as_bytes().to_vec()));

        assert_eq!(reader.next_frame(), None);
        assert!(reader.is_incomplete());
    }

    #[test]
    fn reader_restarts_on_nested_header() {
        let stream = concat!(
            "[JFP_HEADER]\nVERSION: v5.4.0\nSTATUS: OK\n[MESSAGE_BODY]\nurwana\n",
            "[JFP_HEADER]\nVERSION: v5.4.0\nINTENT: TICK\n[END_HEADER]\n[MESSAGE_BODY]\n3\n[END_MESSAGE]\n",
        );

        let mut reader = JfpReader::new(Cursor::new(stream.as_bytes().to_vec()));

        let frame = reader.next_frame().expect("second header wins");
        assert_eq!(frame.intent.as_deref(), Some("TICK"));
        assert_eq!(frame.body, "3");
    }

    #[test]
    fn test_rapid_sequence_reading() {
        let msg = JfpMessage::reply("OK", "payload-data");
        let encoded = msg.to_jfp_string();
        let stream = format!("{encoded}\n{encoded}\n{encoded}\n");

        let mut reader = JfpReader::new(Cursor::new(stream.into_bytes()));
        for _ in 0..3 {
            let parsed = reader.next_frame().expect("valid frame");
            assert_eq!(parsed.status, "OK");
            assert_eq!(parsed.body, "payload-data");
        }
        assert_eq!(reader.next_frame(), None);
    }
}
