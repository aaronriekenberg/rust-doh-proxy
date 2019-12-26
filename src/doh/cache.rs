use tokio::sync::Mutex;

use trust_dns_proto::op::Message;

use std::borrow::{Borrow, BorrowMut};
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
    cache: Mutex<lru::LruCache<String, CacheObject>>,
}

impl Cache {
    pub fn new() -> Self {
        Cache {
            cache: Mutex::new(lru::LruCache::new(10_000)),
        }
    }

    pub async fn get(&self, key: &String) -> Option<CacheObject> {
        let mut guard = self.cache.lock().await;

        let mut_cache = guard.borrow_mut();

        match mut_cache.get(key) {
            Some(v) => Some(v.clone()),
            None => None,
        }
    }

    pub async fn put(&self, key: String, cache_object: CacheObject) -> usize {
        let mut guard = self.cache.lock().await;

        let mut_cache = guard.borrow_mut();

        mut_cache.put(key, cache_object);

        mut_cache.len()
    }

    pub async fn len(&self) -> usize {
        let guard = self.cache.lock().await;

        let map = guard.borrow();

        map.len()
    }
}
