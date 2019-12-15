use tokio::sync::Mutex;

use trust_dns_proto::op::Message;

use std::borrow::{Borrow, BorrowMut};
use std::collections::HashMap;

pub fn get_cache_key(message: &Message) -> String {
    let mut first = true;
    let mut key = String::new();

    for query in message.queries() {
        if !first {
            key.push('|');
        }
        key.push_str(&query.name().to_string());
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
    pub message: Message,
}

impl CacheObject {
    pub fn new(message: Message) -> Self {
        CacheObject { message }
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
