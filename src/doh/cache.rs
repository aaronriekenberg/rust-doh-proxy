use tokio::sync::Mutex;

use trust_dns_proto::op::Message;

use std::borrow::{Borrow, BorrowMut};
use std::collections::HashMap;
use std::time::{Duration, Instant};

pub fn get_cache_key(message: &Message) -> String {
    let mut first = true;
    let mut key = String::new();

    for query in message.queries() {
        if !first {
            key.push('|');
        }
        key.push_str(&query.name().to_string().to_lowercase());
        key.push(':');
        key.push_str(&u16::from(query.query_type()).to_string());
        key.push(':');
        key.push_str(&u16::from(query.query_class()).to_string());
        first = false;
    }

    key
}

#[derive(Clone)]
pub struct CacheObject {
    message: Message,
    cache_time: Instant,
    expiration_time: Instant,
}

impl CacheObject {
    pub fn new(message: Message, cache_time: Instant, expiration_time: Instant) -> Self {
        CacheObject {
            message,
            cache_time,
            expiration_time,
        }
    }

    pub fn message(self) -> Message {
        self.message
    }

    pub fn mut_message(&mut self) -> &mut Message {
        &mut self.message
    }

    pub fn expired(&self) -> bool {
        Instant::now() > self.expiration_time
    }

    pub fn duration_in_cache(&self) -> Duration {
        self.cache_time.elapsed()
    }
}

pub struct Cache {
    map: Mutex<HashMap<String, CacheObject>>,
}

impl Cache {
    pub fn new() -> Self {
        Cache {
            map: Mutex::new(HashMap::new()),
        }
    }

    pub async fn get(&self, key: &String) -> Option<CacheObject> {
        let guard = self.map.lock().await;

        let map = guard.borrow();

        match map.get(key) {
            Some(v) => Some(v.clone()),
            None => None,
        }
    }

    pub async fn put(&self, key: String, cache_object: CacheObject) -> usize {
        let mut guard = self.map.lock().await;

        let mut_map = guard.borrow_mut();

        mut_map.insert(key, cache_object);

        mut_map.len()
    }
}
