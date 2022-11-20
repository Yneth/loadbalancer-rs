use std::future::Future;
use std::io;
use std::io::ErrorKind;
use std::pin::Pin;
use std::task::{Context, Poll, ready};
use std::time::Duration;

use tokio::io::{AsyncRead, AsyncWrite};
use tokio::time::{Instant, Sleep};

use crate::copy::{CopyBuffer, Source};

pub enum TransferResult {
    SourceEOF,
    TargetEOF,
    SourceErr(io::Error),
    TargetErr(io::Error),
}


struct Transfer<'a, A: ?Sized, B: ?Sized> {
    a: &'a mut A,
    b: &'a mut B,
    a_to_b: CopyBuffer,
    b_to_a: CopyBuffer,
    timeout_duration: Duration,
    timeout_fut: Pin<Box<Sleep>>,
}

fn transfer_one_direction<A, B>(
    cx: &mut Context<'_>,
    buf: &mut CopyBuffer,
    r: &mut A,
    w: &mut B,
) -> Poll<Result<u64, (Source, io::Error)>>
    where
        A: AsyncRead + AsyncWrite + Unpin + ?Sized,
        B: AsyncRead + AsyncWrite + Unpin + ?Sized,
{
    let mut r = Pin::new(r);
    let mut w = Pin::new(w);

    buf.poll_copy(cx, r.as_mut(), w.as_mut())
}

impl<'a, A, B> Future for Transfer<'a, A, B>
    where
        A: AsyncRead + AsyncWrite + Unpin + ?Sized,
        B: AsyncRead + AsyncWrite + Unpin + ?Sized,
{
    type Output = TransferResult;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        // Unpack self into mut refs to each field to avoid borrow check issues.
        let Transfer {
            a,
            b,
            a_to_b,
            b_to_a,
            timeout_duration,
            timeout_fut,
        } = &mut *self;

        match timeout_fut.as_mut().poll(cx) {
            Poll::Ready(_) => return Poll::Ready(TransferResult::SourceErr(ErrorKind::TimedOut.into())),
            Poll::Pending => timeout_fut.as_mut().reset(Instant::now() + *timeout_duration),
        }

        let a_to_b = {
            tracing::info_span!("src -> dst").in_scope(||
                transfer_one_direction(cx, a_to_b, &mut *a, &mut *b))
        };

        let b_to_a = {
            tracing::info_span!("dst -> src").in_scope(||
                transfer_one_direction(cx, b_to_a, &mut *b, &mut *a))
        };

        // buf.poll_copy completes only in cases when
        //  * waiting for new data to arrive
        //  * we have an error
        //  * EOF
        // due to the first point waiting for a_to_b may never complete
        // as for proxy connection we are assuming that connection should
        // live as long as possible.
        // which may completely block us from retrieving EOF from target.

        // check without waiting
        match a_to_b {
            Poll::Ready(Err((Source::Reader, e))) => return Poll::Ready(TransferResult::SourceErr(e)),
            Poll::Ready(Err((Source::Writer, e))) => return Poll::Ready(TransferResult::TargetErr(e)),
            Poll::Ready(Ok(_)) => return Poll::Ready(TransferResult::SourceEOF),
            Poll::Pending => {}
        }

        // wait
        match ready!(b_to_a) {
            Err((Source::Reader, e)) => Poll::Ready(TransferResult::TargetErr(e)),
            Err((Source::Writer, e)) => Poll::Ready(TransferResult::SourceErr(e)),
            Ok(_) => Poll::Ready(TransferResult::TargetEOF),
        }
    }
}

/// copy of tokio::io::copy_bidirectional with a modification
/// to stop copying as soon as at least one stream sends EOF
pub async fn transfer<A, B>(a: &mut A, b: &mut B, inactivity_timeout: Duration) -> TransferResult
    where
        A: AsyncRead + AsyncWrite + Unpin + ?Sized,
        B: AsyncRead + AsyncWrite + Unpin + ?Sized,
{
    Transfer {
        a,
        b,
        a_to_b: CopyBuffer::new(),
        b_to_a: CopyBuffer::new(),
        timeout_duration: inactivity_timeout,
        timeout_fut: Box::pin(tokio::time::sleep(inactivity_timeout)),
    }
        .await
}
