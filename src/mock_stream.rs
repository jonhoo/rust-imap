use std::io::{Read, Result, Write, Error, ErrorKind};

pub struct MockStream {
    read_buf: Vec<u8>,
    read_pos: usize,
    pub written_buf: Vec<u8>
}

impl MockStream {
    pub fn new(read_buf: Vec<u8>) -> MockStream {
        MockStream{
            read_buf: read_buf,
            read_pos: 0,
            written_buf: Vec::new()
        }
    }
}

impl Read for MockStream {
    fn read(&mut self, buf: &mut[u8]) -> Result<usize> {
        if self.read_pos >= self.read_buf.len() {
            return Err(Error::new(ErrorKind::UnexpectedEof, "EOF"))
        }
        let write_len = min(buf.len(), self.read_buf.len() - self.read_pos);
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

fn min(a: usize, b: usize) -> usize {
    if a < b {
        a
    } else if b < a {
        b
    } else {
        a
    }
}
