//! Client for the privileged host configuration broker and its read-only port
//! map. The broker owns allocation; this type only reads the published map and
//! asks for a port when an app is launched through a device IP.
use rumahl_platform_web::AppPortProvider;
use std::collections::HashMap;
use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{Duration, Instant};

const MAX_RESPONSE: usize = 8192;
const CACHE_TTL: Duration = Duration::from_secs(1);

pub struct BrokerAppPorts {
    socket: PathBuf,
    ports_file: PathBuf,
    cache: Mutex<Option<(Instant, HashMap<String, u16>)>>,
}

impl BrokerAppPorts {
    pub fn new(socket: PathBuf, ports_file: PathBuf) -> Self {
        Self {
            socket,
            ports_file,
            cache: Mutex::new(None),
        }
    }

    fn load(&self) -> HashMap<String, u16> {
        let mut cache = self
            .cache
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if let Some((at, map)) = cache.as_ref()
            && at.elapsed() < CACHE_TTL
        {
            return map.clone();
        }
        let map: HashMap<String, u16> = std::fs::read(&self.ports_file)
            .ok()
            .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok())
            .and_then(|value| {
                value
                    .get("ports")
                    .and_then(|ports| ports.as_object())
                    .map(|ports| {
                        ports
                            .iter()
                            .filter_map(|(installation, port)| {
                                port.as_u64()
                                    .and_then(|port| u16::try_from(port).ok())
                                    .map(|port| (installation.clone(), port))
                            })
                            .collect()
                    })
            })
            .unwrap_or_default();
        *cache = Some((Instant::now(), map.clone()));
        map
    }

    fn request(&self, value: &serde_json::Value) -> Option<serde_json::Value> {
        let mut stream = UnixStream::connect(&self.socket).ok()?;
        stream
            .set_read_timeout(Some(Duration::from_secs(10)))
            .ok()?;
        stream
            .set_write_timeout(Some(Duration::from_secs(10)))
            .ok()?;
        serde_json::to_writer(&mut stream, value).ok()?;
        stream.write_all(b"\n").ok()?;
        stream.flush().ok()?;
        let mut response = Vec::new();
        let mut chunk = [0_u8; 1024];
        loop {
            let read = stream.read(&mut chunk).ok()?;
            if read == 0 {
                break;
            }
            response.extend_from_slice(&chunk[..read]);
            if response.len() > MAX_RESPONSE {
                return None;
            }
            if chunk[..read].contains(&b'\n') {
                break;
            }
        }
        serde_json::from_slice(&response).ok()
    }
}

impl AppPortProvider for BrokerAppPorts {
    fn ensure_port(&self, installation: &str) -> Option<u16> {
        if let Some(port) = self.port_for(installation) {
            return Some(port);
        }
        let response = self.request(&serde_json::json!({
            "op": "allocate",
            "installation": installation,
        }))?;
        if response.get("ok").and_then(|value| value.as_bool()) != Some(true) {
            return None;
        }
        let port = response
            .get("port")
            .and_then(|value| value.as_u64())
            .and_then(|port| u16::try_from(port).ok())?;
        if let Ok(mut cache) = self.cache.lock() {
            *cache = None;
        }
        Some(port)
    }

    fn port_for(&self, installation: &str) -> Option<u16> {
        self.load().get(installation).copied()
    }

    fn installation_for_port(&self, port: u16) -> Option<String> {
        self.load()
            .into_iter()
            .find(|(_, candidate)| *candidate == port)
            .map(|(installation, _)| installation)
    }

    fn all_ports(&self) -> Vec<u16> {
        let mut ports: Vec<u16> = self.load().into_values().collect();
        ports.sort_unstable();
        ports.dedup();
        ports
    }
}
