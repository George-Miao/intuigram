use std::io::Write;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::thread::JoinHandle;

use flume::{Receiver, Sender, TrySendError};
use futures_util::Stream;
use snafu::{OptionExt, ResultExt};

use super::{
    GraphicsRequest, PrepareSnafu, Result, StartWorkerSnafu, WorkerStoppedSnafu, WriteSnafu,
};

struct Job {
    requests: Vec<GraphicsRequest>,
}

struct Reply {
    requests: Vec<GraphicsRequest>,
    result: rasterm::Result<Vec<u8>>,
}

pub(crate) struct GraphicsWorker {
    jobs: Option<Sender<Job>>,
    replies: flume::r#async::RecvStream<'static, Reply>,
    thread: Option<JoinHandle<()>>,
    desired: Vec<GraphicsRequest>,
    rendered: Vec<GraphicsRequest>,
    in_flight: bool,
    ready: Option<Reply>,
}

impl GraphicsWorker {
    pub(crate) fn new(protocol: rasterm::Protocol) -> Result<Self> {
        let (jobs, job_receiver) = flume::bounded(1);
        let (reply_sender, replies) = flume::bounded(1);
        let thread = std::thread::Builder::new()
            .name("intuigram-graphics".to_owned())
            .spawn(move || run(protocol, &job_receiver, &reply_sender))
            .context(StartWorkerSnafu)?;
        Ok(Self {
            jobs: Some(jobs),
            replies: replies.into_stream(),
            thread: Some(thread),
            desired: Vec::new(),
            rendered: Vec::new(),
            in_flight: false,
            ready: None,
        })
    }

    pub(crate) fn request(&mut self, requests: &[GraphicsRequest]) -> Result<()> {
        self.desired = requests.to_vec();
        self.dispatch()
    }

    pub(crate) fn take_output(&mut self) -> Result<Option<Vec<u8>>> {
        self.poll_with_noop();
        let Some(reply) = self.ready.take() else {
            return Ok(None);
        };
        self.in_flight = false;
        let output = reply.result.context(PrepareSnafu)?;
        self.rendered = reply.requests;
        self.dispatch()?;
        Ok(Some(output))
    }

    pub(crate) fn write(writer: &mut impl Write, output: &[u8]) -> Result<()> {
        writer.write_all(output).context(WriteSnafu)?;
        writer.flush().context(WriteSnafu)
    }

    pub(crate) fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<()>> {
        if self.ready.is_some() {
            return Poll::Ready(Ok(()));
        }
        match Pin::new(&mut self.replies).poll_next(cx) {
            Poll::Ready(Some(reply)) => {
                self.ready = Some(reply);
                Poll::Ready(Ok(()))
            }
            Poll::Ready(None) if self.in_flight => Poll::Ready(WorkerStoppedSnafu.fail()),
            Poll::Ready(None) | Poll::Pending => Poll::Pending,
        }
    }

    fn poll_with_noop(&mut self) {
        let waker = futures_util::task::noop_waker();
        let mut cx = Context::from_waker(&waker);
        let _ = self.poll_ready(&mut cx);
    }

    fn dispatch(&mut self) -> Result<()> {
        if self.in_flight || self.desired == self.rendered {
            return Ok(());
        }
        let job = Job {
            requests: self.desired.clone(),
        };
        match self
            .jobs
            .as_ref()
            .context(WorkerStoppedSnafu)?
            .try_send(job)
        {
            Ok(()) => self.in_flight = true,
            Err(TrySendError::Full(_)) => {}
            Err(TrySendError::Disconnected(_)) => return WorkerStoppedSnafu.fail(),
        }
        Ok(())
    }
}

impl Drop for GraphicsWorker {
    fn drop(&mut self) {
        self.jobs.take();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

fn run(protocol: rasterm::Protocol, jobs: &Receiver<Job>, replies: &Sender<Reply>) {
    let mut renderer = rasterm::Renderer::new(protocol);
    while let Ok(job) = jobs.recv() {
        let mut output = Vec::new();
        let result = renderer.sync(&mut output, &job.requests).map(|()| output);
        if replies
            .send(Reply {
                requests: job.requests,
                result,
            })
            .is_err()
        {
            break;
        }
    }
}
