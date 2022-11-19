use std::io;
use std::pin::Pin;
use std::task::{Context, Poll, ready};

use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};

use crate::consts;

#[derive(Debug)]
pub struct CopyBuffer {
    read_done: bool,
    need_flush: bool,
    pos: usize,
    cap: usize,
    amt: u64,
    buf: Box<[u8]>,
}

pub enum Source {
    Reader,
    Writer,
}

impl CopyBuffer {
    pub fn new() -> Self {
        Self {
            read_done: false,
            need_flush: false,
            pos: 0,
            cap: 0,
            amt: 0,
            buf: vec![0; consts::DEFAULT_BUF_SIZE].into_boxed_slice(),
        }
    }

    pub fn poll_copy<R, W>(
        &mut self,
        cx: &mut Context<'_>,
        mut reader: Pin<&mut R>,
        mut writer: Pin<&mut W>,
    ) -> Poll<Result<u64, (Source, io::Error)>>
        where
            R: AsyncRead + ?Sized,
            W: AsyncWrite + ?Sized,
    {
        loop {
            // If our buffer is empty, then we need to read some data to
            // continue.
            if self.pos == self.cap && !self.read_done {
                let me = &mut *self;
                let mut buf = ReadBuf::new(&mut me.buf);

                tracing::trace!("read");
                match reader.as_mut().poll_read(cx, &mut buf) {
                    Poll::Ready(Ok(_)) => (),
                    Poll::Ready(Err(err)) => return Poll::Ready(Err((Source::Reader, err))),
                    Poll::Pending => {
                        // Try flushing when the reader has no progress to avoid deadlock
                        // when the reader depends on buffered writer.
                        if self.need_flush {
                            ready!(writer.as_mut().poll_flush(cx))
                                .map_err(|e| (Source::Writer, e))?;

                            self.need_flush = false;
                        }

                        return Poll::Pending;
                    }
                }

                let n = buf.filled().len();
                tracing::trace!("read: {}b", n);

                if n == 0 {
                    self.read_done = true;
                } else {
                    self.pos = 0;
                    self.cap = n;
                }
            }

            // If our buffer has some data, let's write it out!
            while self.pos < self.cap {
                let me = &mut *self;
                tracing::trace!("writing: {}b", me.cap - me.pos);

                let i = ready!(writer.as_mut().poll_write(cx, &me.buf[me.pos..me.cap]))
                    .map_err(|e| (Source::Writer, e))?;
                tracing::trace!("wrote: {}b", i);

                if i == 0 {
                    return Poll::Ready(Err((Source::Writer, io::Error::new(
                        io::ErrorKind::WriteZero,
                        "write zero byte into writer",
                    ))));
                } else {
                    self.pos += i;
                    self.amt += i as u64;
                    self.need_flush = true;
                }
            }

            // If pos larger than cap, this loop will never stop.
            // In particular, user's wrong poll_write implementation returning
            // incorrect written length may lead to thread blocking.
            debug_assert!(
                self.pos <= self.cap,
                "writer returned length larger than input slice"
            );

            // If we've written all the data and we've seen EOF, flush out the
            // data and finish the transfer.
            if self.pos == self.cap && self.read_done {
                ready!(writer.as_mut().poll_flush(cx))
                    .map_err(|e| (Source::Writer, e))?;

                return Poll::Ready(Ok(self.amt));
            }
        }
    }
}
