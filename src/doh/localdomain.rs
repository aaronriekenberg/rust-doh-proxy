use crate::doh::cache::{get_cache_key, CacheKey};
use crate::doh::config::{ForwardDomainConfiguration, ReverseDomainConfiguration};

use log::info;

use std::collections::HashMap;
use std::error::Error;
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
    ) -> Result<Self, Box<dyn Error>> {
        let mut cache = HashMap::new();

        for forward_domain_configuration in forward_domain_configurations {
            let message = forward_domain_configuration_to_message(forward_domain_configuration)?;
            cache.insert(get_cache_key(&message), message);
        }

        for reverse_domain_configuration in reverse_domain_configurations {
            let message = reverse_domain_configuration_to_message(reverse_domain_configuration)?;
            cache.insert(get_cache_key(&message), message);
        }

        info!("created local domain cache len {}", cache.len());

        Ok(LocalDomainCache { cache })
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
) -> Result<Message, Box<dyn Error>> {
    let name = Name::from_str(&forward_domain_configuration.name())
        .map_err(|e| format!("invalid forward name: {}", e))?;

    let ip_address = forward_domain_configuration.ip_address().parse()?;

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

    Ok(message)
}

fn reverse_domain_configuration_to_message(
    reverse_domain_configuration: ReverseDomainConfiguration,
) -> Result<Message, Box<dyn Error>> {
    let reverse_address = Name::from_str(&reverse_domain_configuration.reverse_address())
        .map_err(|e| format!("invalid reverse_address: {}", e))?;

    let name = Name::from_str(&reverse_domain_configuration.name())
        .map_err(|e| format!("invalid reverse name: {}", e))?;

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

    Ok(message)
}
