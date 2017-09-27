use std::io::{Read, Result, Write, Error, ErrorKind};
use std::cmp::min;

pub struct MockStream {
    read_buf: Vec<u8>,
    read_pos: usize,
    pub written_buf: Vec<u8>,
    err_on_read: bool,
    eof_on_read: bool,
    read_delay: usize,
}

impl Default for MockStream {
    fn default() -> Self {
        MockStream {
            read_buf: Vec::new(),
            read_pos: 0,
            written_buf: Vec::new(),
            err_on_read: false,
            eof_on_read: false,
            read_delay: 0,
        }
    }
}

impl MockStream {
    pub fn new(read_buf: Vec<u8>) -> MockStream {
        MockStream::default().with_buf(read_buf)
    }

    pub fn with_buf(mut self, read_buf: Vec<u8>) -> MockStream {
        self.read_buf = read_buf;
        self
    }

    pub fn with_eof(mut self) -> MockStream {
        self.eof_on_read = true;
        self
    }

    pub fn with_err(mut self) -> MockStream {
        self.err_on_read = true;
        self
    }

    pub fn with_delay(mut self) -> MockStream {
        self.read_delay = 1;
        self
    }
}

impl Read for MockStream {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        if self.eof_on_read {
            return Ok(0);
        }
        if self.err_on_read {
            return Err(Error::new(ErrorKind::Other, "MockStream Error"));
        }
        if self.read_pos >= self.read_buf.len() {
            return Err(Error::new(ErrorKind::UnexpectedEof, "EOF"));
        }
        let mut write_len = min(buf.len(), self.read_buf.len() - self.read_pos);
        if self.read_delay > 0 {
            self.read_delay -= 1;
            write_len = min(write_len, 1);
        }
        let max_pos = self.read_pos + write_len;
        for x in self.read_pos..max_pos {
            buf[x - self.read_pos] = self.read_buf[x];
        }
        self.read_pos += write_len;
        Ok(write_len)
    }
}

impl Write for MockStream {
    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        self.written_buf.extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> Result<()> {
        Ok(())
    }
}
