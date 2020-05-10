use crate::doh::config::CacheConfiguration;

use tokio::sync::Mutex;

use trust_dns_proto::op::Message;

use std::convert::TryFrom;
use std::time::{Duration, Instant};

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct CacheKey {
    key: String,
}

impl TryFrom<&Message> for CacheKey {
    type Error = &'static str;

    fn try_from(message: &Message) -> Result<Self, Self::Error> {
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

        if key.is_empty() {
            Err("key string is empty")
        } else {
            Ok(CacheKey { key })
        }
    }
}

#[derive(Clone)]
pub struct CacheObject {
    message: Message,
    cache_time: Instant,
    expiration_time: Instant,
}

impl CacheObject {
    pub fn new(message: Message, cache_time: Instant, cache_duration: Duration) -> Self {
        let expiration_time = cache_time + cache_duration;
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

    pub fn duration_in_cache(&self, now: Instant) -> Duration {
        now - self.cache_time
    }
}

pub struct Cache {
    cache_configuration: CacheConfiguration,
    cache: Mutex<lru::LruCache<CacheKey, CacheObject>>,
}

impl Cache {
    pub fn new(cache_configuration: CacheConfiguration) -> Self {
        let max_size = cache_configuration.max_size();

        Cache {
            cache_configuration,
            cache: Mutex::new(lru::LruCache::new(max_size)),
        }
    }

    pub async fn get(&self, key: &CacheKey) -> Option<CacheObject> {
        let mut mut_cache = self.cache.lock().await;

        match mut_cache.get(key) {
            Some(v) => Some(v.clone()),
            None => None,
        }
    }

    pub async fn put(&self, key: CacheKey, cache_object: CacheObject) {
        let mut mut_cache = self.cache.lock().await;

        mut_cache.put(key, cache_object);
    }

    pub async fn periodic_purge(&self) -> (usize, usize) {
        let mut mut_cache = self.cache.lock().await;

        let mut items_purged = 0;

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

        (mut_cache.len(), items_purged)
    }
}
