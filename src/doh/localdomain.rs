use crate::doh::cache::{get_cache_key, CacheKey};
use crate::doh::config::{ForwardDomainConfiguration, ReverseDomainConfiguration};

use std::collections::HashMap;
use std::str::FromStr;

use trust_dns_proto::op::{Message, MessageType, Query, ResponseCode};
use trust_dns_proto::rr::resource::Record;
use trust_dns_proto::rr::{Name, RData, RecordType};

pub struct LocalDomainCache {
    cache: HashMap<CacheKey, Message>,
}

impl LocalDomainCache {
    pub fn new(
        forward_domain_configurations: Vec<ForwardDomainConfiguration>,
        reverse_domain_configurations: Vec<ReverseDomainConfiguration>,
    ) -> Self {
        let mut cache = HashMap::new();

        for forward_domain_configuration in forward_domain_configurations {
            let message = forward_domain_configuration_to_message(forward_domain_configuration);
            cache.insert(get_cache_key(&message), message);
        }

        for reverse_domain_configuration in reverse_domain_configurations {
            let message = reverse_domain_configuration_to_message(reverse_domain_configuration);
            cache.insert(get_cache_key(&message), message);
        }

        LocalDomainCache { cache }
    }

    pub fn get_response_message(&self, cache_key: &CacheKey) -> Option<Message> {
        match self.cache.get(cache_key) {
            None => None,
            Some(message) => Some(message.clone()),
        }
    }
}

fn forward_domain_configuration_to_message(
    forward_domain_configuration: ForwardDomainConfiguration,
) -> Message {
    let name =
        Name::from_str(&forward_domain_configuration.name()).expect("invalid forward domain name");
    let ip_address = forward_domain_configuration
        .ip_address()
        .parse()
        .expect("invalid forward domain ip address");

    let mut message = Message::new();
    message.set_message_type(MessageType::Response);
    message.set_response_code(ResponseCode::NoError);
    message.set_authoritative(true);

    let query = Query::query(name.clone(), RecordType::A);
    message.add_query(query);

    let answer = Record::from_rdata(
        name,
        forward_domain_configuration.ttl_seconds(),
        RData::A(ip_address),
    );
    message.add_answer(answer);

    message
}

fn reverse_domain_configuration_to_message(
    reverse_domain_configuration: ReverseDomainConfiguration,
) -> Message {
    let reverse_address = Name::from_str(&reverse_domain_configuration.reverse_address())
        .expect("invalid reverse address");
    let name = Name::from_str(&reverse_domain_configuration.name()).expect("invalid reverse name");

    let mut message = Message::new();
    message.set_message_type(MessageType::Response);
    message.set_response_code(ResponseCode::NoError);
    message.set_authoritative(true);

    let query = Query::query(reverse_address.clone(), RecordType::PTR);
    message.add_query(query);

    let answer = Record::from_rdata(
        reverse_address,
        reverse_domain_configuration.ttl_seconds(),
        RData::PTR(name),
    );
    message.add_answer(answer);

    message
}
