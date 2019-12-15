use tokio::sync::Mutex;

use trust_dns_proto::op::Message;

use std::borrow::BorrowMut;
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

pub struct CacheObject {
    message: Message,
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

    pub async fn put(&mut self, key: String, cacheObject: CacheObject) {
        let mut guard = self.map.lock().await;

        let map = guard.borrow_mut();

        map.insert(key, cacheObject);
    }
}
