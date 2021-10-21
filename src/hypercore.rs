use futures::channel::mpsc::UnboundedSender as Sender;
use futures::lock::Mutex;
use futures::sink::SinkExt;
use futures::stream::StreamExt;
use hypercore::{Feed, Node, Proof, PublicKey, Signature};
use hypercore_protocol::schema::*;
use hypercore_protocol::{discovery_key, Channel, Duplex, Event, Message, Protocol};
use log::*;
use pretty_hash::fmt as pretty_fmt;
use random_access_storage::RandomAccess;
use std::collections::{BTreeMap, HashMap};
use std::convert::{TryFrom, TryInto};
use std::fmt::Debug;
use std::io;
use std::sync::Arc;
use wasm_bindgen_futures::spawn_local;

use crate::persistence::{RandomAccessProxy, WasmStorage};
use crate::ws::{ReadHalf, WriteHalf};
use crate::AppEvent;

fn parse_key_from_string(key: &str) -> anyhow::Result<[u8; 32]> {
    let key = hex::decode(key.as_bytes())?;
    let key = key
        .try_into()
        .map_err(|_| anyhow::anyhow!("Invalid key length"))?;
    Ok(key)
}

pub async fn replicate(
    mut protocol: Protocol<Duplex<ReadHalf, WriteHalf>>,
    key: impl AsRef<str>,
    app_tx: Sender<AppEvent>,
) -> anyhow::Result<()> {
    let mut feedstore = FeedStore::new();
    let storage = WasmStorage::new_proxy().await.unwrap();
    let key = parse_key_from_string(key.as_ref())?;
    let public_key = PublicKey::from_bytes(&key).unwrap();
    let feed = Feed::builder(public_key, storage).build().await.unwrap();
    let feed_wrapper = FeedWrapper::from_proxy_feed(feed);
    feedstore.add(feed_wrapper);
    let feedstore = Arc::new(feedstore);

    while let Some(event) = protocol.next().await {
        let event = event.expect("Protocol error");
        match event {
            Event::Handshake(_remote_public_key) => {
                debug!("received handshake from remote");
            }
            Event::DiscoveryKey(discovery_key) => {
                if let Some(feed) = feedstore.get(&discovery_key) {
                    debug!(
                        "open discovery_key: {}",
                        pretty_fmt(&discovery_key).unwrap()
                    );
                    protocol.open(feed.key).await.unwrap();
                } else {
                    debug!(
                        "unknown discovery_key: {}",
                        pretty_fmt(&discovery_key).unwrap()
                    );
                }
            }
            Event::Channel(channel) => {
                if let Some(feed) = feedstore.get(channel.discovery_key()) {
                    let mut app_tx = app_tx.clone();
                    feed.on_peer(channel, &mut app_tx);
                    // let feed = feed.clone();
                    // let mut app_tx = app_tx.clone();
                    // feed.on_open(&mut channel).await.unwrap();
                    // spawn_local(async move {
                    //     feed.on_peer()
                    //     while let Some(message) = channel.next().await {
                    //         feed.on_message(&mut channel, message, &mut app_tx)
                    //             .await
                    //             .unwrap();
                    //     }
                    // });
                }
            }
            _ => {}
        }
    }
    Ok(())
}

/// A Feed is a single unit of replication, an append-only log.
#[derive(Debug, Clone)]
struct FeedWrapper<T>
where
    T: RandomAccess<Error = Box<dyn std::error::Error + Send + Sync>> + Debug + Send,
{
    discovery_key: [u8; 32],
    key: [u8; 32],
    feed: Arc<Mutex<Feed<T>>>,
}

impl FeedWrapper<RandomAccessProxy> {
    pub fn from_proxy_feed(feed: Feed<RandomAccessProxy>) -> Self {
        let key = feed.public_key().to_bytes();
        FeedWrapper {
            key,
            discovery_key: discovery_key(&key),
            feed: Arc::new(Mutex::new(feed)),
        }
    }

    pub fn on_peer(&self, mut channel: Channel, app_tx: &mut Sender<AppEvent>) {
        let mut feed = self.feed.clone();
        let mut app_tx = app_tx.clone();
        spawn_local(async move {
            let msg = Want {
                start: 0,
                length: None,
            };
            channel.send(Message::Want(msg)).await.unwrap();
            let mut state = FeedState::default();
            while let Some(message) = channel.next().await {
                let result =
                    on_message(&mut feed, &mut state, &mut channel, message, &mut app_tx).await;
                if let Err(e) = result {
                    error!("protocol error: {}", e);
                    break;
                }
            }
        });
    }
}

struct FeedStore {
    feeds: HashMap<String, Arc<FeedWrapper<RandomAccessProxy>>>,
}
impl FeedStore {
    pub fn new() -> Self {
        let feeds = HashMap::new();
        Self { feeds }
    }

    pub fn add(&mut self, feed: FeedWrapper<RandomAccessProxy>) {
        let hdkey = hex::encode(&feed.discovery_key);
        self.feeds.insert(hdkey, Arc::new(feed));
    }

    pub fn get(&self, discovery_key: &[u8]) -> Option<&Arc<FeedWrapper<RandomAccessProxy>>> {
        let hdkey = hex::encode(discovery_key);
        self.feeds.get(&hdkey)
    }
}

/// A Feed is a single unit of replication, an append-only log.
/// This toy feed can only read sequentially and does not save or buffer anything.
// #[derive(Debug)]
// struct ToyFeed {
//     key: [u8; 32],
//     discovery_key: [u8; 32],
//     state: Mutex<FeedState>,
// }
// impl ToyFeed {
//     pub fn new(key: [u8; 32]) -> Self {
//         ToyFeed {
//             discovery_key: discovery_key(&key),
//             key,
//             state: Mutex::new(FeedState::default()),
//         }
//     }
async fn on_message(
    feed: &mut Arc<Mutex<Feed<RandomAccessProxy>>>,
    state: &mut FeedState,
    channel: &mut Channel,
    message: Message,
    app_tx: &mut Sender<AppEvent>,
) -> io::Result<()> {
    // debug!("receive message: {:?}", message);
    match message {
        Message::Have(message) => on_have(feed, state, channel, message).await,
        Message::Data(message) => on_data(feed, state, channel, message, app_tx).await,
        _ => Ok(()),
    }
}

// async fn on_open(
//     feed: &mut Arc<Mutex<Feed<RandomAccessProxy>>>,
//     state: Mutex<FeedState>,
//     channel: &mut Channel,
// ) -> io::Result<()> {
//     log::info!(
//         "open channel {}",
//         pretty_fmt(channel.discovery_key()).unwrap()
//     );
//     let msg = Want {
//         start: 0,
//         length: None,
//     };
//     channel.want(msg).await
// }

async fn on_have(
    feed: &mut Arc<Mutex<Feed<RandomAccessProxy>>>,
    state: &mut FeedState,
    channel: &mut Channel,
    msg: Have,
) -> io::Result<()> {
    feed.lock().await;
    // Check if the remote announces a new head.
    log::info!("receive have: {} (state {})", msg.start, state.remote_head);
    if state.remote_head == 0 || msg.start > state.remote_head {
        // Store the new remote head.
        state.remote_head = msg.start;
        // If we didn't start reading, request first data block.
        if !state.started {
            state.started = true;
            let msg = Request {
                index: 0,
                bytes: None,
                hash: None,
                nodes: None,
            };
            channel.request(msg).await?;
        }
    }
    Ok(())
}

async fn on_data(
    feed: &mut Arc<Mutex<Feed<RandomAccessProxy>>>,
    state: &mut FeedState,
    channel: &mut Channel,
    msg: Data,
    app_tx: &mut Sender<AppEvent>,
) -> io::Result<()> {
    let mut feed = feed.lock().await;
    log::info!(
        "receive data: idx {}, {} bytes (remote_head {})",
        msg.index,
        msg.value.as_ref().map_or(0, |v| v.len()),
        state.remote_head
    );

    let value: Option<&[u8]> = match msg.value.as_ref() {
        None => None,
        Some(value) => {
            debug!("receive data: {:?}", String::from_utf8(value.clone()));
            Some(value)
        }
    };

    let signature = match msg.signature {
        Some(bytes) => Some(Signature::try_from(&bytes[..]).unwrap()),
        None => None,
    };
    let nodes = msg
        .nodes
        .iter()
        .map(|n| Node::new(n.index, n.hash.clone(), n.size))
        .collect();
    let proof = Proof {
        index: msg.index,
        nodes,
        signature,
    };

    feed.put(msg.index, value, proof.clone()).await.unwrap();

    let next = msg.index + 1;
    if state.remote_head >= next {
        // Request next data block.
        let msg = Request {
            index: next,
            bytes: None,
            hash: None,
            nodes: None,
        };
        channel.request(msg).await?;
    } else {
        let mut full_text = String::new();
        for i in 0..feed.len() {
            let line = String::from_utf8(feed.get(i).await.unwrap().unwrap()).unwrap();
            full_text.push_str(&line);
        }
        app_tx
            .send(AppEvent::ContentLoaded(full_text))
            .await
            .unwrap();
    }

    Ok(())
}

/// A FeedState stores the head seq of the remote.
/// This would have a bitfield to support sparse sync in the actual impl.
#[derive(Debug)]
struct FeedState {
    pub remote_head: u64,
    pub started: bool,
}
impl Default for FeedState {
    fn default() -> Self {
        FeedState {
            remote_head: 0,
            started: false,
        }
    }
}
