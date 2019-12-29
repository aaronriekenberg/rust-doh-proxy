use log::{debug, warn};

use trust_dns_proto::error::ProtoResult;
use trust_dns_proto::op::Message;
use trust_dns_proto::serialize::binary::{BinDecodable, BinDecoder, BinEncodable, BinEncoder};

pub fn encode_dns_message(message: &Message) -> ProtoResult<Vec<u8>> {
    let mut request_buffer = Vec::new();

    let mut encoder = BinEncoder::new(&mut request_buffer);
    match message.emit(&mut encoder) {
        Ok(_) => {
            debug!(
                "encoded message request_buffer.len = {}",
                request_buffer.len()
            );
            Ok(request_buffer)
        }
        Err(e) => {
            warn!("error encoding message request buffer {}", e);
            Err(e)
        }
    }
}

pub fn decode_dns_message_slice(buffer: &[u8]) -> ProtoResult<Message> {
    let mut decoder = BinDecoder::new(&buffer);
    match Message::read(&mut decoder) {
        Ok(message) => Ok(message),
        Err(e) => {
            warn!("error decoding dns message {}", e);
            Err(e)
        }
    }
}

pub fn decode_dns_message_vec(buffer: Vec<u8>) -> ProtoResult<Message> {
    decode_dns_message_slice(&buffer)
}
