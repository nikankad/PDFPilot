//! Independent local review service. Bind is loopback-only; remote deployment
//! needs a TLS reverse proxy, operational access controls and backups.
use crate::pdf::{self, PdfDocument};
use anyhow::{Context, Result, ensure};
use rand::Rng;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, SystemTime, UNIX_EPOCH},
};

const LIMIT: usize = 65_536;
fn token() -> String {
    let mut bytes = [0u8; 32];
    rand::rng().fill(&mut bytes);
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
fn equals(a: &str, b: &str) -> bool {
    a.len() == b.len() && a.bytes().zip(b.bytes()).fold(0u8, |v, (a, b)| v | (a ^ b)) == 0
}
fn valid_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 64 && id.bytes().all(|c| c.is_ascii_hexdigit())
}
#[derive(Clone)]
pub struct Service {
    root: PathBuf,
    admin: String,
    lock: Arc<Mutex<()>>,
}
pub struct Running {
    pub address: String,
    stop: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}
impl Drop for Running {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}
impl Service {
    pub fn open(root: &Path) -> Result<Self> {
        std::fs::create_dir_all(root)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(root, std::fs::Permissions::from_mode(0o700))?;
        }
        let config = root.join("config.json");
        let admin = if config.exists() {
            let v: Value = serde_json::from_slice(&std::fs::read(config)?)?;
            v["admin"]
                .as_str()
                .context("Invalid service config")?
                .to_string()
        } else {
            let admin = token();
            write_json(&config, &json!({"admin":admin}))?;
            admin
        };
        Ok(Self {
            root: root.to_owned(),
            admin,
            lock: Arc::new(Mutex::new(())),
        })
    }
    pub fn publish(
        &self,
        doc: &lopdf::Document,
        title: &str,
        recipients: &[String],
        ordered: bool,
    ) -> Result<Value> {
        let _guard = self
            .lock
            .lock()
            .map_err(|_| anyhow::anyhow!("Service lock unavailable"))?;
        ensure!(
            title.len() <= 500
                && !title.chars().any(char::is_control)
                && recipients.len() <= 100
                && recipients.iter().all(|s| !s.trim().is_empty()
                    && s.len() <= 200
                    && !s.chars().any(char::is_control)),
            "Invalid title or recipients"
        );
        ensure!(
            doc.encryption_state.is_none(),
            "Remove authorized password protection before publishing a review"
        );
        let id = token()[..32].to_string();
        let folder = self.root.join(&id);
        std::fs::create_dir(&folder)?;
        let path = folder.join("source.pdf");
        pdf::atomic_save(doc, &path)?;
        let review_token = token();
        let recipients = recipients
            .iter()
            .map(|name| json!({"name":name,"token":token(),"signed":false}))
            .collect::<Vec<_>>();
        let mut state = json!({"id":id,"title":title,"pages":doc.get_pages().len(),"review_token":review_token,"source_sha256":digest(&std::fs::read(path)?),"ordered":ordered,"recipients":recipients,"comments":[],"events":[]});
        event(
            &mut state,
            "created",
            json!({"recipients":recipients.len()}),
        )?;
        write_json(&folder.join("state.json"), &state)?;
        Ok(state)
    }
    fn load(&self, id: &str) -> Result<Value> {
        ensure!(valid_id(id), "Invalid review identifier");
        let state: Value =
            serde_json::from_slice(&std::fs::read(self.root.join(id).join("state.json"))?)?;
        verify_audit(&state)?;
        ensure!(
            digest(&std::fs::read(self.root.join(id).join("source.pdf"))?)
                == state["source_sha256"].as_str().unwrap_or_default(),
            "Published document changed"
        );
        Ok(state)
    }
    pub fn snapshot(&self, id: &str) -> Result<Value> {
        let _guard = self
            .lock
            .lock()
            .map_err(|_| anyhow::anyhow!("Service lock unavailable"))?;
        self.load(id)
    }
    pub fn remind(&self, id: &str) -> Result<()> {
        let _guard = self
            .lock
            .lock()
            .map_err(|_| anyhow::anyhow!("Service lock unavailable"))?;
        let mut state = self.load(id)?;
        let remaining = state["recipients"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|r| r["signed"] != true)
            .map(|r| r["name"].clone())
            .collect::<Vec<_>>();
        event(&mut state, "reminder", json!({"recipients":remaining}))?;
        write_json(&self.root.join(id).join("state.json"), &state)
    }
    fn role(state: &Value, auth: &str) -> Option<usize> {
        state["recipients"]
            .as_array()?
            .iter()
            .position(|r| equals(r["token"].as_str().unwrap_or_default(), auth))
    }
    fn authorize(&self, state: &Value, auth: &str) -> bool {
        equals(auth, &self.admin)
            || equals(auth, state["review_token"].as_str().unwrap_or_default())
            || Self::role(state, auth).is_some()
    }
    pub fn comment(&self, id: &str, auth: &str, page: u32, author: &str, text: &str) -> Result<()> {
        let _guard = self
            .lock
            .lock()
            .map_err(|_| anyhow::anyhow!("Service lock unavailable"))?;
        let mut state = self.load(id)?;
        ensure!(self.authorize(&state, auth), "Unauthorized");
        ensure!(
            page > 0
                && page as u64 <= state["pages"].as_u64().unwrap_or(0)
                && !text.trim().is_empty()
                && text.len() <= 10_000
                && author.len() <= 200,
            "Invalid comment"
        );
        let comment = json!({"id":token()[..16],"page":page,"author":author,"text":text,"time":now(),"resolved":false});
        state["comments"]
            .as_array_mut()
            .unwrap()
            .push(comment.clone());
        event(&mut state, "comment", comment)?;
        write_json(&self.root.join(id).join("state.json"), &state)
    }
    pub fn resolve_comment(
        &self,
        id: &str,
        auth: &str,
        comment_id: &str,
        resolved: bool,
    ) -> Result<()> {
        let _guard = self
            .lock
            .lock()
            .map_err(|_| anyhow::anyhow!("Service lock unavailable"))?;
        let mut state = self.load(id)?;
        ensure!(self.authorize(&state, auth), "Unauthorized");
        let comment = state["comments"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|c| c["id"] == comment_id)
            .context("Unknown comment")?;
        comment["resolved"] = json!(resolved);
        event(
            &mut state,
            "comment_status",
            json!({"comment":comment_id,"resolved":resolved}),
        )?;
        write_json(&self.root.join(id).join("state.json"), &state)
    }
    pub fn sign(&self, id: &str, auth: &str, typed_name: &str) -> Result<()> {
        let _guard = self
            .lock
            .lock()
            .map_err(|_| anyhow::anyhow!("Service lock unavailable"))?;
        let mut state = self.load(id)?;
        let index = Self::role(&state, auth).context("A recipient signing link is required")?;
        ensure!(
            !typed_name.trim().is_empty()
                && typed_name.len() <= 200
                && !typed_name.chars().any(char::is_control),
            "Enter your typed signature"
        );
        let recipients = state["recipients"].as_array().unwrap();
        ensure!(
            recipients[index]["signed"] != true,
            "Recipient already signed"
        );
        if state["ordered"] == true {
            ensure!(
                recipients[..index].iter().all(|r| r["signed"] == true),
                "Earlier recipients must sign first"
            );
        }
        let name = recipients[index]["name"].clone();
        state["recipients"][index]["signed"] = json!(true);
        state["recipients"][index]["typed_signature"] = json!(typed_name);
        state["recipients"][index]["signed_at"] = json!(now());
        let source_hash = state["source_sha256"].clone();
        event(
            &mut state,
            "signed",
            json!({"recipient":name,"typed_signature":typed_name,"source_sha256":source_hash}),
        )?;
        write_json(&self.root.join(id).join("state.json"), &state)
    }
    pub fn completed_pdf(&self, id: &str, output: &Path) -> Result<()> {
        let _guard = self
            .lock
            .lock()
            .map_err(|_| anyhow::anyhow!("Service lock unavailable"))?;
        let state = self.load(id)?;
        let recipients = state["recipients"].as_array().unwrap();
        ensure!(
            !recipients.is_empty() && recipients.iter().all(|r| r["signed"] == true),
            "The signing request is not complete"
        );
        let body = format!(
            "Document: {}\nSource SHA-256: {}\n\n{}\n\nElectronic signature receipt. Possession of a signing link authenticates the request token, not the person's real-world identity.\nNo certificate signature or trusted timestamp is applied.",
            state["title"].as_str().unwrap_or("Document"),
            state["source_sha256"].as_str().unwrap_or_default(),
            recipients
                .iter()
                .map(|r| format!(
                    "{}: {} / UTC Unix time {}",
                    r["name"].as_str().unwrap_or_default(),
                    r["typed_signature"].as_str().unwrap_or_default(),
                    r["signed_at"]
                ))
                .collect::<Vec<_>>()
                .join("\n")
        );
        // The receipt's standard PDF font is Western Latin. Escape unsupported
        // codepoints without losing identity data, and attach exact UTF-8 evidence.
        let display = body
            .chars()
            .map(|c| {
                if c.is_ascii() {
                    c.to_string()
                } else {
                    c.escape_unicode().to_string()
                }
            })
            .collect::<String>();
        let receipt = pdf::create_text_document(
            "PDFPilot signing receipt",
            &format!(
                "{display}\nUnicode characters are shown as codepoints. Exact names and history are in the attached signing-record.json."
            ),
        )?;
        let temp = tempfile::tempdir()?;
        let path = temp.path().join("receipt.pdf");
        pdf::atomic_save(&receipt, &path)?;
        let mut doc = PdfDocument::open(&self.root.join(id).join("source.pdf"), "")?;
        doc.merge(&path)?;
        let mut evidence = state.clone();
        evidence.as_object_mut().unwrap().remove("review_token");
        for recipient in evidence["recipients"].as_array_mut().unwrap() {
            recipient.as_object_mut().unwrap().remove("token");
        }
        let evidence_path = temp.path().join("signing-record.json");
        std::fs::write(&evidence_path, serde_json::to_vec_pretty(&evidence)?)?;
        doc.attach_file(&evidence_path)?;
        doc.save_copy(output)
    }
    pub fn start(&self, port: u16) -> Result<Running> {
        let listener = TcpListener::bind(("127.0.0.1", port))?;
        let address = format!("http://{}", listener.local_addr()?);
        listener.set_nonblocking(true)?;
        let stop = Arc::new(AtomicBool::new(false));
        let cancelled = stop.clone();
        let service = self.clone();
        let thread = std::thread::spawn(move || {
            while !cancelled.load(Ordering::Relaxed) {
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        // macOS can inherit the listener's nonblocking flag.
                        // Request parsing uses bounded blocking reads.
                        if stream.set_nonblocking(false).is_err() {
                            continue;
                        }
                        let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
                        let _ = stream.set_write_timeout(Some(Duration::from_secs(2)));
                        if let Err(e) = service.http(&mut stream) {
                            let status = if e.to_string().contains("Unauthorized") {
                                401
                            } else {
                                400
                            };
                            let _ = respond(
                                &mut stream,
                                status,
                                "application/json",
                                &serde_json::to_vec(&json!({"error":e.to_string()}))
                                    .unwrap_or_default(),
                            );
                        }
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(Duration::from_millis(25))
                    }
                    Err(_) => break,
                }
            }
        });
        Ok(Running {
            address,
            stop,
            thread: Some(thread),
        })
    }
    fn http(&self, stream: &mut TcpStream) -> Result<()> {
        let request = read_request(stream)?;
        let path = request.path.split('?').next().unwrap_or("");
        if request.method == "GET" && path.starts_with("/review/") {
            let id = path.trim_start_matches("/review/");
            ensure!(valid_id(id), "Invalid review identifier");
            return respond(
                stream,
                200,
                "text/html; charset=utf-8",
                include_bytes!("service/review.html"),
            );
        }
        let pieces = path
            .split('/')
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>();
        ensure!(
            pieces.len() >= 3 && pieces[0] == "api" && pieces[1] == "reviews",
            "Unknown endpoint"
        );
        let id = pieces[2];
        let state = self.snapshot(id)?;
        ensure!(self.authorize(&state, &request.auth), "Unauthorized");
        if pieces.len() == 3 && request.method == "GET" {
            let mut visible = state.clone();
            visible.as_object_mut().unwrap().remove("review_token");
            for r in visible["recipients"].as_array_mut().unwrap() {
                r.as_object_mut().unwrap().remove("token");
            }
            visible["can_sign"] = json!(Self::role(&state, &request.auth));
            return respond(
                stream,
                200,
                "application/json",
                &serde_json::to_vec(&visible)?,
            );
        }
        let operation = pieces.get(3).copied().unwrap_or("");
        match (request.method.as_str(), operation) {
            ("GET", "document.pdf") => respond(
                stream,
                200,
                "application/pdf",
                &std::fs::read(self.root.join(id).join("source.pdf"))?,
            ),
            ("GET", "completed.pdf") => {
                let temp = tempfile::tempdir()?;
                let path = temp.path().join("signed.pdf");
                self.completed_pdf(id, &path)?;
                respond(stream, 200, "application/pdf", &std::fs::read(path)?)
            }
            ("POST", "comments") => {
                let v: Value = serde_json::from_slice(&request.body)?;
                let page = u32::try_from(v["page"].as_u64().context("Page is required")?)?;
                self.comment(
                    id,
                    &request.auth,
                    page,
                    v["author"].as_str().unwrap_or("Reviewer"),
                    v["text"].as_str().context("Comment text required")?,
                )?;
                respond(stream, 200, "application/json", b"{}")
            }
            ("POST", "resolve") => {
                let v: Value = serde_json::from_slice(&request.body)?;
                self.resolve_comment(
                    id,
                    &request.auth,
                    v["comment_id"].as_str().context("Comment ID required")?,
                    v["resolved"]
                        .as_bool()
                        .context("Resolved status required")?,
                )?;
                respond(stream, 200, "application/json", b"{}")
            }
            ("POST", "sign") => {
                let v: Value = serde_json::from_slice(&request.body)?;
                ensure!(v["consent"] == true, "Explicit signing consent is required");
                self.sign(
                    id,
                    &request.auth,
                    v["typed_name"]
                        .as_str()
                        .context("Typed signature required")?,
                )?;
                respond(stream, 200, "application/json", b"{}")
            }
            _ => anyhow::bail!("Unknown endpoint"),
        }
    }
}
fn write_json(path: &Path, value: &Value) -> Result<()> {
    let mut file =
        tempfile::NamedTempFile::new_in(path.parent().context("Missing parent folder")?)?;
    file.write_all(&serde_json::to_vec_pretty(value)?)?;
    file.as_file().sync_all()?;
    file.persist(path).map_err(|e| e.error)?;
    Ok(())
}
fn event(state: &mut Value, kind: &str, data: Value) -> Result<()> {
    let events = state["events"]
        .as_array_mut()
        .context("Missing audit history")?;
    let previous = events.last().and_then(|e| e["hash"].as_str()).unwrap_or("");
    let mut e =
        json!({"index":events.len(),"time":now(),"kind":kind,"data":data,"previous":previous});
    let hash = digest(&serde_json::to_vec(&e)?);
    e["hash"] = json!(hash);
    events.push(e);
    Ok(())
}
pub fn verify_audit(state: &Value) -> Result<()> {
    let mut previous = String::new();
    for (index, event) in state["events"]
        .as_array()
        .context("Missing audit history")?
        .iter()
        .enumerate()
    {
        ensure!(
            event["index"] == index && event["previous"] == previous,
            "Audit chain is broken"
        );
        let mut data = event.clone();
        let hash = data
            .as_object_mut()
            .context("Invalid audit event")?
            .remove("hash")
            .context("Audit hash missing")?;
        ensure!(
            hash == digest(&serde_json::to_vec(&data)?),
            "Audit event was modified"
        );
        previous = hash.as_str().context("Invalid audit hash")?.into();
    }
    Ok(())
}
struct Request {
    method: String,
    path: String,
    auth: String,
    body: Vec<u8>,
}
fn read_request(stream: &mut TcpStream) -> Result<Request> {
    let mut header = vec![];
    let mut byte = [0];
    while !header.ends_with(b"\r\n\r\n") {
        ensure!(header.len() < 8192, "HTTP headers too large");
        stream.read_exact(&mut byte)?;
        header.push(byte[0]);
    }
    let text = std::str::from_utf8(&header)?;
    let mut lines = text.split("\r\n");
    let first = lines
        .next()
        .context("Missing HTTP request")?
        .split_whitespace()
        .collect::<Vec<_>>();
    ensure!(
        first.len() == 3 && first[2] == "HTTP/1.1",
        "Unsupported HTTP request"
    );
    let mut length = None;
    let mut auth = String::new();
    for line in lines.filter(|s| !s.is_empty()) {
        let (key, value) = line.split_once(':').context("Invalid HTTP header")?;
        ensure!(
            !key.eq_ignore_ascii_case("transfer-encoding"),
            "Chunked requests unsupported"
        );
        if key.eq_ignore_ascii_case("content-length") {
            ensure!(length.is_none(), "Duplicate Content-Length");
            length = Some(value.trim().parse::<usize>()?);
        }
        if key.eq_ignore_ascii_case("authorization") {
            ensure!(auth.is_empty(), "Duplicate Authorization");
            auth = value
                .trim()
                .strip_prefix("Bearer ")
                .context("Bearer token required")?
                .into();
        }
    }
    let length = length.unwrap_or(0);
    ensure!(length <= LIMIT, "HTTP body too large");
    let mut body = vec![0; length];
    stream.read_exact(&mut body)?;
    Ok(Request {
        method: first[0].into(),
        path: first[1].into(),
        auth,
        body,
    })
}
fn respond(stream: &mut TcpStream, status: u16, kind: &str, body: &[u8]) -> Result<()> {
    write!(
        stream,
        "HTTP/1.1 {status} {}\r\nContent-Type: {kind}\r\nContent-Length: {}\r\nConnection: close\r\nCache-Control: no-store\r\nReferrer-Policy: no-referrer\r\nX-Content-Type-Options: nosniff\r\nX-Frame-Options: DENY\r\nContent-Security-Policy: default-src 'self'; script-src 'self' 'unsafe-inline'; style-src 'self' 'unsafe-inline'; object-src blob:; frame-src blob:; connect-src 'self'; base-uri 'none'; frame-ancestors 'none'\r\n\r\n",
        if status == 200 { "OK" } else { "Bad Request" },
        body.len()
    )?;
    stream.write_all(body)?;
    Ok(())
}
