use trust_dns_proto::op::Message;

use std::convert::TryFrom;

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct RequestKey {
    key: String,
}

impl TryFrom<&Message> for RequestKey {
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
            Ok(RequestKey { key })
        }
    }
}
