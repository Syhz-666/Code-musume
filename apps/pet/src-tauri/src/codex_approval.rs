//! Read-only observer of the desktop's local thread sync protocol.
use code_musume_core::engine::CodexApprovals;
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, io, time::Duration};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::windows::named_pipe::{ClientOptions, NamedPipeClient},
    sync::{mpsc, watch},
    time::{interval, sleep, timeout, Instant, MissedTickBehavior},
};

pub const PIPE: &str = r"\\.\pipe\codex-ipc";
const SYNC_VERSION: u64 = 11;
const MAX_FRAME: usize = 32 * 1024 * 1024;
const RETRY: Duration = Duration::from_secs(2);
const WRITE_TIMEOUT: Duration = Duration::from_secs(2);

pub type Update = (String, Option<CodexApprovals>);

#[derive(Clone, Copy)]
enum Shape {
    Root,
    Requests,
    Request,
    Params,
    Method,
    Blocking,
    Turns,
    TurnHistory,
    History,
    Entities,
    Turn,
    Items,
    Item,
    ItemType,
    ReviewStatus,
    TurnStatus,
}

impl Shape {
    fn element(self) -> Option<Self> {
        match self {
            Self::Requests => Some(Self::Request),
            Self::Turns => Some(Self::Turn),
            Self::Items => Some(Self::Item),
            _ => None,
        }
    }

    fn fields(self) -> &'static [(&'static str, Self)] {
        match self {
            Self::Root => &[
                ("requests", Self::Requests),
                ("turns", Self::Turns),
                ("turnHistory", Self::TurnHistory),
            ],
            Self::TurnHistory => &[("history", Self::History)],
            Self::History => &[("entitiesByKey", Self::Entities)],
            Self::Request => &[("method", Self::Method), ("params", Self::Params)],
            Self::Params => &[("isBlocking", Self::Blocking)],
            Self::Turn => &[("items", Self::Items), ("status", Self::TurnStatus)],
            Self::Item => &[("type", Self::ItemType), ("status", Self::ReviewStatus)],
            _ => &[],
        }
    }

    fn child(self, key: &Value) -> Option<Self> {
        if let Some(element) = self.element() {
            return (key.is_u64() || key.as_str() == Some("-")).then_some(element);
        }
        let key = key.as_str()?;
        if matches!(self, Self::Entities) {
            return (!key.is_empty() && key.len() <= 512).then_some(Self::Turn);
        }
        self.fields()
            .iter()
            .find(|(field, _)| *field == key)
            .map(|(_, shape)| *shape)
    }

    // Keep array positions for desktop patches, but discard all text, IDs and
    // fields outside the approval schema before retaining a snapshot or delta.
    fn redact(self, value: &Value) -> Value {
        if matches!(self, Self::Entities) {
            return match value.as_object() {
                Some(entries) => Value::Object(
                    entries
                        .iter()
                        .map(|(key, turn)| (entity_key(key), Self::Turn.redact(turn)))
                        .collect(),
                ),
                None => Value::Null,
            };
        }
        if let Some(element) = self.element() {
            return match value.as_array() {
                Some(values) => Value::Array(values.iter().map(|v| element.redact(v)).collect()),
                None => Value::Null,
            };
        }
        if value.is_object() && !self.fields().is_empty() {
            return Value::Object(
                self.fields()
                    .iter()
                    .filter_map(|(key, shape)| {
                        value.get(*key).map(|v| ((*key).into(), shape.redact(v)))
                    })
                    .collect::<Map<_, _>>(),
            );
        }
        match (self, value.as_str()) {
            (
                Self::Method,
                Some(
                    "item/commandExecution/requestApproval"
                    | "item/fileChange/requestApproval"
                    | "item/permissions/requestApproval"
                    | "item/tool/requestUserInput",
                ),
            )
            | (Self::ItemType, Some("automaticApprovalReview"))
            | (Self::ReviewStatus, Some("inProgress"))
            | (Self::TurnStatus, Some("completed" | "failed" | "interrupted" | "cancelled")) => {
                value.clone()
            }
            (Self::Blocking, _) if value.is_boolean() => value.clone(),
            _ => Value::Null,
        }
    }
}

fn entity_key(key: &str) -> String {
    format!("{:x}", Sha256::digest(key.as_bytes()))
}

fn apply(state: &mut Value, patch: &Value) -> Option<()> {
    let path = patch.get("path")?.as_array()?;
    let mut shape = Shape::Root;
    let mut pointer = String::new();
    for (index, part) in path.iter().enumerate() {
        if shape.element().is_some()
            && part.as_str() == Some("length")
            && index + 1 == path.len()
            && patch["op"] == "replace"
        {
            let length = usize::try_from(patch["value"].as_u64()?).ok()?;
            let array = state.pointer_mut(&pointer)?.as_array_mut()?;
            if length > array.len() {
                return None;
            }
            array.truncate(length);
            return Some(());
        }
        let Some(child) = shape.child(part) else {
            return Some(());
        };
        pointer.push('/');
        match part {
            Value::String(key) if matches!(shape, Shape::Entities) => {
                pointer.push_str(&entity_key(key));
            }
            Value::String(key) => pointer.push_str(key),
            _ => pointer.push_str(&part.to_string()),
        }
        shape = child;
    }
    let op = match patch["op"].as_str()? {
        op @ ("add" | "replace") => {
            json!({"op":op,"path":pointer,"value":shape.redact(patch.get("value")?)})
        }
        "remove" => json!({"op":"remove","path":pointer}),
        _ => return None,
    };
    let op: json_patch::PatchOperation = serde_json::from_value(op).ok()?;
    json_patch::patch(state, &[op]).ok()
}

fn counts(state: &Value) -> Option<CodexApprovals> {
    let mut result = CodexApprovals {
        manual: 0,
        human_input: 0,
        automatic: 0,
    };
    for request in state.get("requests")?.as_array()? {
        match request["method"].as_str() {
            Some("item/tool/requestUserInput") => {
                if request["params"]["isBlocking"] != false {
                    result.human_input += 1;
                }
            }
            Some(_) => result.manual += 1,
            None => {}
        }
    }
    // Paged Codex chats use entitiesByKey, while legacy chats use turns.
    let entities = state
        .pointer("/turnHistory/history/entitiesByKey")
        .and_then(Value::as_object);
    let legacy = if entities.is_none() {
        Some(state.get("turns")?.as_array()?)
    } else {
        None
    };
    for turn in legacy
        .into_iter()
        .flatten()
        .chain(entities.into_iter().flat_map(Map::values))
    {
        if !turn["status"].is_null() {
            continue;
        }
        if let Some(items) = turn["items"].as_array() {
            result.automatic += items
                .iter()
                .filter(|item| {
                    item["type"] == "automaticApprovalReview" && item["status"] == "inProgress"
                })
                .count();
        }
    }
    Some(result)
}

struct Stream {
    owner: String,
    revision: u64,
    state: Value,
    counts: CodexApprovals,
}

#[derive(Default)]
struct Observer {
    streams: BTreeMap<String, Option<Stream>>,
}

enum Observation {
    Changed(Update),
    Resync(String),
}

impl Observer {
    fn frame(&mut self, msg: &Value) -> Vec<Observation> {
        if msg["type"] != "broadcast" {
            return vec![];
        }
        let params = &msg["params"];
        let Some(owner) = msg["sourceClientId"].as_str() else {
            return vec![];
        };
        if msg["method"] == "client-status-changed" && params["status"] == "disconnected" {
            return self
                .streams
                .iter_mut()
                .filter_map(|(id, stream)| {
                    if stream.as_ref().is_some_and(|s| s.owner == owner) {
                        *stream = None;
                        Some(Observation::Changed((id.clone(), None)))
                    } else {
                        None
                    }
                })
                .collect();
        }
        if msg["method"] != "thread-stream-state-changed" || params["hostId"] != "local" {
            return vec![];
        }
        let Some(id) = params["conversationId"].as_str() else {
            return vec![];
        };
        let Some(current) = self.streams.get_mut(id) else {
            return vec![];
        };
        if current.as_ref().is_some_and(|s| s.owner != owner) {
            return vec![];
        }
        let change = &params["change"];
        let previous = current.as_ref().map(|s| s.counts);
        let invalidate = |current: &mut Option<Stream>, retry_now| {
            if current.take().is_some() {
                let mut updates = vec![Observation::Changed((id.into(), None))];
                if retry_now {
                    updates.push(Observation::Resync(id.into()));
                }
                updates
            } else {
                vec![]
            }
        };
        if msg["version"] != SYNC_VERSION {
            return invalidate(current, false);
        }
        let Some(revision) = change["revision"].as_u64() else {
            return invalidate(current, false);
        };
        match change["type"].as_str() {
            Some("snapshot") => {
                if current.as_ref().is_some_and(|s| revision <= s.revision) {
                    return vec![];
                }
                let state = Shape::Root.redact(&change["conversationState"]);
                let Some(counts) = counts(&state) else {
                    return invalidate(current, false);
                };
                *current = Some(Stream {
                    owner: owner.into(),
                    revision,
                    state,
                    counts,
                });
            }
            Some("patches") => {
                let Some(stream) = current.as_mut() else {
                    return vec![];
                };
                if revision <= stream.revision {
                    return vec![];
                }
                if change["baseRevision"].as_u64() != Some(stream.revision) {
                    return invalidate(current, true);
                }
                let Some(patches) = change["patches"].as_array() else {
                    return invalidate(current, true);
                };
                if patches
                    .iter()
                    .any(|patch| apply(&mut stream.state, patch).is_none())
                {
                    return invalidate(current, true);
                }
                let Some(counts) = counts(&stream.state) else {
                    return invalidate(current, true);
                };
                stream.revision = revision;
                stream.counts = counts;
            }
            _ => return invalidate(current, false),
        }
        let next = current.as_ref().map(|stream| stream.counts);
        if previous == next {
            vec![]
        } else {
            vec![Observation::Changed((id.into(), next))]
        }
    }
}

async fn send(pipe: &mut NamedPipeClient, msg: Value) -> io::Result<()> {
    let body = serde_json::to_vec(&msg)?;
    let mut frame = (body.len() as u32).to_le_bytes().to_vec();
    frame.extend_from_slice(&body);
    timeout(WRITE_TIMEOUT, pipe.write_all(&frame)).await?
}

async fn follow(
    pipe: &mut NamedPipeClient,
    client: &str,
    id: &str,
    following: bool,
) -> io::Result<()> {
    send(
        pipe,
        json!({
            "type":"broadcast","sourceClientId":client,"method":"thread-stream-following-changed",
            "version":1,"params":{"conversationId":id,"hostId":"local","following":following}
        }),
    )
    .await
}

async fn sync(
    pipe: &mut NamedPipeClient,
    client: &str,
    observer: &mut Observer,
    wanted: &[String],
) -> io::Result<()> {
    let removed: Vec<_> = observer
        .streams
        .keys()
        .filter(|id| !wanted.contains(id))
        .cloned()
        .collect();
    for id in removed {
        follow(pipe, client, &id, false).await?;
        observer.streams.remove(&id);
    }
    for id in wanted {
        if observer.streams.get(id).is_none_or(Option::is_none) {
            follow(pipe, client, id, true).await?;
            observer.streams.entry(id.clone()).or_default();
        }
    }
    Ok(())
}

#[derive(Default)]
struct Frames(Vec<u8>);

impl Frames {
    fn next(&mut self) -> io::Result<Option<Value>> {
        let Some(header) = self.0.get(..4) else {
            return Ok(None);
        };
        let len = u32::from_le_bytes(header.try_into().unwrap()) as usize;
        if len == 0 || len > MAX_FRAME {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "invalid frame size",
            ));
        }
        let Some(body) = self.0.get(4..4 + len) else {
            return Ok(None);
        };
        let value = serde_json::from_slice(body)?;
        self.0.drain(..4 + len);
        Ok(Some(value))
    }
}

async fn connected(
    mut pipe: NamedPipeClient,
    wanted: &mut watch::Receiver<Vec<String>>,
    updates: &mpsc::Sender<Update>,
) -> io::Result<()> {
    let init = uuid::Uuid::new_v4().to_string();
    send(
        &mut pipe,
        json!({
            "type":"request","requestId":init,"sourceClientId":"unassigned","method":"initialize",
            "version":1,"params":{"clientType":"code-musume-observer"}
        }),
    )
    .await?;
    let mut frames = Frames::default();
    let mut observer = Observer::default();
    let mut client = None::<String>;
    let mut chunk = [0; 16 * 1024];
    let mut retry = interval(RETRY);
    retry.set_missed_tick_behavior(MissedTickBehavior::Delay);
    let start = Instant::now();
    loop {
        tokio::select! {
            read = pipe.read(&mut chunk) => {
                let size = read?;
                if size == 0 { return Ok(()) }
                frames.0.extend_from_slice(&chunk[..size]);
                while let Some(msg) = frames.next()? {
                    if client.is_none() && msg["requestId"] == init && msg["type"] == "response"
                        && msg["resultType"] == "success" {
                        if let Some(id) = msg["result"]["clientId"].as_str() {
                            client = Some(id.to_owned());
                            let ids = wanted.borrow_and_update().clone();
                            sync(&mut pipe, id, &mut observer, &ids).await?;
                        }
                    } else if let Some(client) = &client {
                        for event in observer.frame(&msg) {
                            match event {
                                Observation::Changed(update) => {
                                    if updates.send(update).await.is_err() { return Ok(()) }
                                }
                                Observation::Resync(id) => follow(&mut pipe, client, &id, true).await?,
                            }
                        }
                    }
                }
            },
            changed = wanted.changed() => {
                if changed.is_err() { return Ok(()) }
                let ids = wanted.borrow_and_update().clone();
                if let Some(client) = &client { sync(&mut pipe, client, &mut observer, &ids).await?; }
            },
            _ = retry.tick() => {
                if let Some(client) = &client {
                    let ids = wanted.borrow_and_update().clone();
                    sync(&mut pipe, client, &mut observer, &ids).await?;
                } else if start.elapsed() > Duration::from_secs(5) {
                    return Err(io::Error::new(io::ErrorKind::TimedOut, "desktop handshake timed out"));
                }
            }
        }
    }
}

pub async fn observe(
    path: &str,
    mut wanted: watch::Receiver<Vec<String>>,
    updates: mpsc::Sender<Update>,
) {
    loop {
        if wanted.wait_for(|ids| !ids.is_empty()).await.is_err() {
            return;
        }
        // Tokio's default named-pipe QoS is SECURITY_IDENTIFICATION. The server
        // can identify this client but cannot impersonate it.
        if let Ok(pipe) = ClientOptions::new().open(path) {
            let _ = connected(pipe, &mut wanted, &updates).await;
            let ids = wanted.borrow().clone();
            for id in ids {
                if updates.send((id, None)).await.is_err() {
                    return;
                }
            }
        }
        sleep(RETRY).await;
    }
}

#[cfg(test)]
mod tests;
