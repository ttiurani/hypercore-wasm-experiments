use futures::channel::mpsc::UnboundedSender as Sender;
use futures::lock::Mutex;
use futures::sink::SinkExt;
use futures::stream::StreamExt;
use hypercore::{Hypercore, PartialKeypair, PublicKey};
use hypercore_protocol::hypercore::RequestUpgrade;
use hypercore_protocol::hypercore::{self, RequestBlock};
use hypercore_protocol::schema::*;
use hypercore_protocol::{discovery_key, Channel, Duplex, Event, Message, Protocol};
use log::*;
use pretty_hash::fmt as pretty_fmt;
use random_access_storage::RandomAccess;
use std::collections::HashMap;
use std::convert::TryInto;
use std::fmt::Debug;
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
    let mut hypercore_store = HypercoreStore::new();
    let storage = WasmStorage::new_proxy().await.unwrap();
    let key = parse_key_from_string(key.as_ref())?;
    let public_key = PublicKey::from_bytes(&key).unwrap();
    let hypercore = Hypercore::new_with_key_pair(
        storage,
        PartialKeypair {
            secret: None,
            public: public_key,
        },
    )
    .await
    .unwrap();
    let hypercore_wrapper = HypercoreWrapper::from_proxy_hypercore(hypercore);
    hypercore_store.add(hypercore_wrapper);
    let hypercore_store = Arc::new(hypercore_store);

    while let Some(event) = protocol.next().await {
        let event = event.expect("Protocol error");
        match event {
            Event::Handshake(_remote_public_key) => {
                debug!("received handshake from remote");
            }
            Event::DiscoveryKey(discovery_key) => {
                if let Some(hypercore) = hypercore_store.get(&discovery_key) {
                    debug!(
                        "open discovery_key: {}",
                        pretty_fmt(&discovery_key).unwrap()
                    );
                    protocol.open(hypercore.key).await.unwrap();
                } else {
                    debug!(
                        "unknown discovery_key: {}",
                        pretty_fmt(&discovery_key).unwrap()
                    );
                }
            }
            Event::Channel(channel) => {
                if let Some(hypercore) = hypercore_store.get(channel.discovery_key()) {
                    let mut app_tx = app_tx.clone();
                    hypercore.on_peer(channel, &mut app_tx);
                }
            }
            _ => {}
        }
    }
    Ok(())
}

/// A Hypercore is a single unit of replication, an append-only log.
#[derive(Debug, Clone)]
struct HypercoreWrapper<T>
where
    T: RandomAccess<Error = Box<dyn std::error::Error + Send + Sync>> + Debug + Send,
{
    discovery_key: [u8; 32],
    key: [u8; 32],
    hypercore: Arc<Mutex<Hypercore<T>>>,
}

impl HypercoreWrapper<RandomAccessProxy> {
    pub fn from_proxy_hypercore(hypercore: Hypercore<RandomAccessProxy>) -> Self {
        let key = hypercore.key_pair().public.to_bytes();
        HypercoreWrapper {
            key,
            discovery_key: discovery_key(&key),
            hypercore: Arc::new(Mutex::new(hypercore)),
        }
    }

    pub fn on_peer(&self, mut channel: Channel, app_tx: &mut Sender<AppEvent>) {
        let mut hypercore = self.hypercore.clone();
        let mut app_tx = app_tx.clone();
        spawn_local(async move {
            let info = {
                let hypercore = hypercore.lock().await;
                hypercore.info()
            };
            let sync_msg = Synchronize {
                fork: info.fork,
                length: info.length,
                remote_length: 0,
                can_upgrade: true,
                uploading: true,
                downloading: true,
            };
            channel.send(Message::Synchronize(sync_msg)).await.unwrap();
            let mut state = PeerState::default();
            while let Some(message) = channel.next().await {
                let result = on_message(
                    &mut hypercore,
                    &mut state,
                    &mut channel,
                    message,
                    &mut app_tx,
                )
                .await;
                if let Err(e) = result {
                    error!("protocol error: {}", e);
                    break;
                }
            }
        });
    }
}

struct HypercoreStore {
    hypercores: HashMap<String, Arc<HypercoreWrapper<RandomAccessProxy>>>,
}
impl HypercoreStore {
    pub fn new() -> Self {
        let hypercores = HashMap::new();
        Self { hypercores }
    }

    pub fn add(&mut self, hypercore: HypercoreWrapper<RandomAccessProxy>) {
        let hdkey = hex::encode(&hypercore.discovery_key);
        self.hypercores.insert(hdkey, Arc::new(hypercore));
    }

    pub fn get(&self, discovery_key: &[u8]) -> Option<&Arc<HypercoreWrapper<RandomAccessProxy>>> {
        let hdkey = hex::encode(discovery_key);
        self.hypercores.get(&hdkey)
    }
}

async fn on_message(
    hypercore: &mut Arc<Mutex<Hypercore<RandomAccessProxy>>>,
    state: &mut PeerState,
    channel: &mut Channel,
    message: Message,
    app_tx: &mut Sender<AppEvent>,
) -> anyhow::Result<()> {
    match message {
        Message::Synchronize(message) => on_synchronize(hypercore, state, channel, message).await,
        Message::Data(message) => on_data(hypercore, state, channel, message, app_tx).await,
        _ => Ok(()),
    }
}

async fn on_synchronize(
    hypercore: &mut Arc<Mutex<Hypercore<RandomAccessProxy>>>,
    peer_state: &mut PeerState,
    channel: &mut Channel,
    message: Synchronize,
) -> anyhow::Result<()> {
    hypercore.lock().await;

    let length_changed = message.length != peer_state.remote_length;
    peer_state.remote_length = message.length;
    peer_state.remote_can_upgrade = message.can_upgrade;
    peer_state.remote_uploading = message.uploading;
    peer_state.remote_downloading = message.downloading;
    peer_state.remote_synced = true;

    let info = {
        let hypercore = hypercore.lock().await;
        hypercore.info()
    };

    let mut messages = vec![];
    if peer_state.remote_synced {
        // Need to send another sync back that acknowledges the received sync
        let msg = Synchronize {
            fork: info.fork,
            length: info.length,
            remote_length: peer_state.remote_length,
            can_upgrade: true, // Real version should handle fork mismatch
            uploading: true,
            downloading: true,
        };
        messages.push(Message::Synchronize(msg));
    }
    if peer_state.remote_length > info.length && length_changed {
        let msg = Request {
            id: 1,
            fork: info.fork,
            hash: None,
            block: None,
            seek: None,
            upgrade: Some(RequestUpgrade {
                start: info.length,
                length: peer_state.remote_length - info.length,
            }),
        };
        messages.push(Message::Request(msg));
    }

    channel.send_batch(&messages).await?;
    Ok(())
}

async fn on_data(
    hypercore: &mut Arc<Mutex<Hypercore<RandomAccessProxy>>>,
    peer_state: &mut PeerState,
    channel: &mut Channel,
    message: Data,
    app_tx: &mut Sender<AppEvent>,
) -> anyhow::Result<()> {
    let (old_info, applied, new_info, synced) = {
        let mut hypercore = hypercore.lock().await;
        let old_info = hypercore.info();
        let proof = message.clone().into_proof();
        let applied = hypercore.verify_and_apply_proof(&proof).await?;
        let new_info = hypercore.info();
        let synced = new_info.contiguous_length == new_info.length;
        (old_info, applied, new_info, synced)
    };
    let mut messages: Vec<Message> = vec![];
    if let Some(upgrade) = &message.upgrade {
        let new_length = upgrade.length;
        let remote_length = peer_state.remote_length;
        messages.push(Message::Synchronize(Synchronize {
            fork: new_info.fork,
            length: new_length,
            remote_length,
            can_upgrade: false,
            uploading: true,
            downloading: true,
        }));

        for i in old_info.length..new_length {
            messages.push(Message::Request(Request {
                id: i + 1,
                fork: new_info.fork,
                hash: None,
                block: Some(RequestBlock { index: i, nodes: 2 }),
                seek: None,
                upgrade: None,
            }));
        }
    }
    if synced {
        let mut full_text = String::new();
        let mut hypercore = hypercore.lock().await;
        for i in 0..new_info.contiguous_length {
            let line = String::from_utf8(hypercore.get(i).await.unwrap().unwrap()).unwrap();
            full_text.push_str(&line);
        }
        app_tx
            .send(AppEvent::ContentLoaded(full_text))
            .await
            .unwrap();
    } else {
        channel.send_batch(&messages).await.unwrap();
    }
    Ok(())
}

/// A PeerState stores the head seq of the remote.
/// This would have a bitfield to support sparse sync in the actual impl.
#[derive(Debug)]
struct PeerState {
    remote_length: u64,
    remote_can_upgrade: bool,
    remote_uploading: bool,
    remote_downloading: bool,
    remote_synced: bool,
}
impl Default for PeerState {
    fn default() -> Self {
        PeerState {
            remote_length: 0,
            remote_can_upgrade: false,
            remote_uploading: true,
            remote_downloading: true,
            remote_synced: false,
        }
    }
}
