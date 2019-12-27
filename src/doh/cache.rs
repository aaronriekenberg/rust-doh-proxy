use crate::doh::config::CacheConfiguration;

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

    pub fn message_mut(&mut self) -> &mut Message {
        &mut self.message
    }

    pub fn expired(&self, now: Instant) -> bool {
        now > self.expiration_time
    }

    pub fn duration_in_cache(&self) -> Duration {
        self.cache_time.elapsed()
    }
}

pub struct Cache {
    cache_configuration: CacheConfiguration,
    cache: Mutex<lru::LruCache<String, CacheObject>>,
}

impl Cache {
    pub fn new(cache_configuration: CacheConfiguration) -> Self {
        let max_size = cache_configuration.max_size();

        Cache {
            cache_configuration,
            cache: Mutex::new(lru::LruCache::new(max_size)),
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

    pub async fn put(&self, key: String, cache_object: CacheObject) {
        let mut guard = self.cache.lock().await;

        let mut_cache = guard.borrow_mut();

        mut_cache.put(key, cache_object);
    }

    pub async fn len(&self) -> usize {
        let guard = self.cache.lock().await;

        let cache = guard.borrow();

        cache.len()
    }

    pub async fn periodic_purge(&self) -> usize {
        let mut guard = self.cache.lock().await;

        let mut_cache = guard.borrow_mut();

        let mut items_purged: usize = 0;

        let now = Instant::now();

        while items_purged < self.cache_configuration.max_purges_per_timer_pop() {
            let lru_key_and_value = match mut_cache.peek_lru() {
                None => break,
                Some(lru_key_and_value) => lru_key_and_value,
            };

            if lru_key_and_value.1.expired(now) {
                let key_clone = lru_key_and_value.0.clone();
                mut_cache.pop(&key_clone);
                items_purged += 1;
            } else {
                break;
            }
        }

        items_purged
    }
}
