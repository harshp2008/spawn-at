//! A scripted stand-in for the Sway IPC socket, for tests (compiled only for tests).
//!
//! It listens on a unix socket in a temporary directory, answers each request with the next
//! payload scripted for its type (the last one repeats), records every request, and after a
//! `SUBSCRIBE` pushes the scripted events on that connection. No Sway is involved.

use super::ipc::{self, MessageType, EVENT_WINDOW};
use std::collections::HashMap;
use std::io::Write;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// What the fake says.
#[derive(Default, Clone)]
pub struct Script {
    /// Reply payloads per request type, in order; the last one is repeated.
    pub replies: HashMap<u32, Vec<Vec<u8>>>,
    /// `window` event payloads pushed after a `SUBSCRIBE` reply, each after its delay.
    pub events: Vec<(Duration, Vec<u8>)>,
    /// Reply with these bytes, verbatim, instead of a frame (a socket that is not Sway).
    pub garbage: Option<Vec<u8>>,
}

impl Script {
    pub fn reply(mut self, kind: MessageType, payload: &[u8]) -> Self {
        self.replies
            .entry(kind as u32)
            .or_default()
            .push(payload.to_vec());
        self
    }

    pub fn event(mut self, after: Duration, payload: &str) -> Self {
        self.events.push((after, payload.as_bytes().to_vec()));
        self
    }
}

pub struct FakeSway {
    pub path: PathBuf,
    requests: Arc<Mutex<Vec<(u32, String)>>>,
    stop: Arc<AtomicBool>,
}

static COUNTER: AtomicUsize = AtomicUsize::new(0);

impl FakeSway {
    pub fn start(script: Script) -> FakeSway {
        let path = std::env::temp_dir().join(format!(
            "spawn-at-fake-sway-{}-{}.sock",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::SeqCst)
        ));
        let _ = std::fs::remove_file(&path);
        let listener = UnixListener::bind(&path).expect("bind the fake socket");
        listener.set_nonblocking(true).unwrap();
        let requests = Arc::new(Mutex::new(Vec::new()));
        let stop = Arc::new(AtomicBool::new(false));
        let queues = Arc::new(Mutex::new(script.replies.clone()));
        let (req2, stop2) = (requests.clone(), stop.clone());
        std::thread::spawn(move || {
            while !stop2.load(Ordering::SeqCst) {
                match listener.accept() {
                    Ok((stream, _)) => {
                        stream.set_nonblocking(false).unwrap();
                        let (script, queues, req) = (script.clone(), queues.clone(), req2.clone());
                        std::thread::spawn(move || serve(stream, script, queues, req));
                    }
                    Err(_) => std::thread::sleep(Duration::from_millis(5)),
                }
            }
        });
        FakeSway {
            path,
            requests,
            stop,
        }
    }

    /// The requests received so far: (type, payload as text).
    pub fn requests(&self) -> Vec<(u32, String)> {
        self.requests.lock().unwrap().clone()
    }

    /// The payloads of the `RUN_COMMAND` requests received so far.
    pub fn commands(&self) -> Vec<String> {
        self.requests()
            .into_iter()
            .filter(|(k, _)| *k == MessageType::RunCommand as u32)
            .map(|(_, p)| p)
            .collect()
    }
}

impl Drop for FakeSway {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        let _ = std::fs::remove_file(&self.path);
    }
}

fn serve(
    mut stream: UnixStream,
    script: Script,
    queues: Arc<Mutex<HashMap<u32, Vec<Vec<u8>>>>>,
    requests: Arc<Mutex<Vec<(u32, String)>>>,
) {
    while let Ok(frame) = ipc::read_frame(&mut stream) {
        requests.lock().unwrap().push((
            frame.kind,
            String::from_utf8_lossy(&frame.payload).into_owned(),
        ));
        if let Some(bytes) = &script.garbage {
            let _ = stream.write_all(bytes);
            continue;
        }
        let payload = {
            let mut q = queues.lock().unwrap();
            match q.get_mut(&frame.kind) {
                Some(list) if list.len() > 1 => list.remove(0),
                Some(list) if list.len() == 1 => list[0].clone(),
                _ => b"[]".to_vec(),
            }
        };
        let mut reply = Vec::new();
        reply.extend_from_slice(ipc::MAGIC);
        reply.extend_from_slice(&(payload.len() as u32).to_ne_bytes());
        reply.extend_from_slice(&frame.kind.to_ne_bytes());
        reply.extend_from_slice(&payload);
        if stream.write_all(&reply).is_err() {
            return;
        }
        if frame.kind == MessageType::Subscribe as u32 {
            for (after, event) in &script.events {
                std::thread::sleep(*after);
                let mut out = Vec::new();
                out.extend_from_slice(ipc::MAGIC);
                out.extend_from_slice(&(event.len() as u32).to_ne_bytes());
                out.extend_from_slice(&EVENT_WINDOW.to_ne_bytes());
                out.extend_from_slice(event);
                if stream.write_all(&out).is_err() {
                    return;
                }
            }
        }
    }
}
