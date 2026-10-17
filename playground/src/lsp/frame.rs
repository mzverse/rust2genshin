//! LSP 的 stdio 拆帧:`Content-Length: <字节数>\r\n\r\n<正文>`。
//!
//! 与 `crate::docker::FrameDecoder` 是两回事 —— 后者解的是 docker CLI attach
//! 协议的 8 字节头。这里解的是 LSP 自己的头,两者不能混用。

/// 头找不到、长度不是数字、或头没写完就收到了正文。
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum FrameError {
    #[error("LSP 帧头不对:{0}")]
    BadHeader(String),
}

/// 组一帧。**长度按字节算**(`str::len()`),不是按字符数:UTF-8 下一个中文
/// 占 3 字节,写成字符数 RA 会截帧。
pub fn encode(msg: &str) -> Vec<u8> {
    let body = msg.as_bytes();
    let mut out = Vec::with_capacity(body.len() + 32);
    out.extend_from_slice(format!("Content-Length: {}\r\n\r\n", body.len()).as_bytes());
    out.extend_from_slice(body);
    out
}

const HEADER_END: &[u8] = b"\r\n\r\n";

#[derive(Default)]
pub struct FrameDecoder {
    buf: Vec<u8>,
}

impl FrameDecoder {
    pub fn new() -> Self {
        Self { buf: Vec::new() }
    }

    /// 喂任意一段字节,吐出里面**已经完整**的正文。
    ///
    /// 半个帧不是错误,只是还没有输出 —— 调用方拿 `Ok(vec![])` 当「还没到齐」。
    /// `BadHeader` 才是真的要断线。
    pub fn push(&mut self, chunk: &[u8]) -> Result<Vec<Vec<u8>>, FrameError> {
        self.buf.extend_from_slice(chunk);
        let mut out = Vec::new();
        loop {
            let Some(hdr_end) = find(&self.buf, HEADER_END) else { break };
            let header = std::str::from_utf8(&self.buf[..hdr_end])
                .map_err(|e| FrameError::BadHeader(format!("头不是 UTF-8:{e}")))?;
            let len = parse_len(header)?;
            let body_start = hdr_end + HEADER_END.len();
            if self.buf.len() < body_start + len {
                break;
            }
            out.push(self.buf[body_start..body_start + len].to_vec());
            self.buf.drain(..body_start + len);
        }
        Ok(out)
    }
}

fn parse_len(header: &str) -> Result<usize, FrameError> {
    for line in header.split("\r\n") {
        if let Some(v) = line.strip_prefix("Content-Length:") {
            return v.trim().parse::<usize>().map_err(|_| FrameError::BadHeader(header.into()));
        }
    }
    Err(FrameError::BadHeader(header.into()))
}

fn find(hay: &[u8], needle: &[u8]) -> Option<usize> {
    hay.windows(needle.len()).position(|w| w == needle)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ok(m: &str) -> Vec<u8> {
        encode(m)
    }

    #[test]
    fn one_frame_whole() {
        let mut d = FrameDecoder::new();
        assert_eq!(d.push(&ok("hello")).unwrap(), vec![b"hello".to_vec()]);
    }

    #[test]
    fn header_split_across_chunks() {
        let full = ok("hello");
        let mut d = FrameDecoder::new();
        assert!(d.push(&full[..5]).unwrap().is_empty());
        assert!(d.push(&full[5..15]).unwrap().is_empty());
        assert_eq!(d.push(&full[15..]).unwrap(), vec![b"hello".to_vec()]);
    }

    #[test]
    fn two_frames_one_chunk() {
        let mut chunk = ok("a");
        chunk.extend(ok("bb"));
        let mut d = FrameDecoder::new();
        assert_eq!(
            d.push(&chunk).unwrap(),
            vec![b"a".to_vec(), b"bb".to_vec()]
        );
    }

    #[test]
    fn utf8_body_survives_split() {
        let full = encode("中文字符串");
        let mut d = FrameDecoder::new();
        let mut out = Vec::new();
        for i in 0..full.len() {
            out.extend(d.push(&full[i..i + 1]).unwrap());
        }
        assert_eq!(out, vec!["中文字符串".as_bytes().to_vec()]);
    }

    #[test]
    fn encode_is_byte_length_not_char_length() {
        // 中文一个字 3 字节,头里写 9 而不是 3
        assert!(encode("中").starts_with(b"Content-Length: 3\r\n"));
    }

    #[test]
    fn bad_header_is_an_error() {
        let mut d = FrameDecoder::new();
        assert!(matches!(
            d.push(b"Content-Length: abc\r\n\r\n{}"),
            Err(FrameError::BadHeader(_))
        ));
    }

    #[test]
    fn body_shorter_than_declared_is_incomplete_not_an_error() {
        let full = encode("hello");
        let mut d = FrameDecoder::new();
        assert!(d.push(&full[..full.len() - 2]).unwrap().is_empty());
    }
}