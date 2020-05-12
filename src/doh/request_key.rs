use trust_dns_proto::op::Message;
use trust_dns_proto::rr::dns_class::DNSClass;
use trust_dns_proto::rr::record_type::RecordType;

use std::convert::TryFrom;

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct RequestQueryKey {
    name: String,
    query_type: RecordType,
    query_class: DNSClass,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct RequestKey {
    query_keys: Vec<RequestQueryKey>,
}

impl TryFrom<&Message> for RequestKey {
    type Error = &'static str;

    fn try_from(message: &Message) -> Result<Self, Self::Error> {
        let mut query_keys = Vec::with_capacity(message.queries().len());

        for query in message.queries() {
            let mut name_string = query.name().to_string();
            name_string.make_ascii_lowercase();

            query_keys.push(RequestQueryKey {
                name: name_string,
                query_type: query.query_type(),
                query_class: query.query_class(),
            });
        }

        if query_keys.is_empty() {
            Err("query_keys is empty")
        } else {
            Ok(RequestKey { query_keys })
        }
    }
}
