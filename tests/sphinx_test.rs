// sphinx_test.rs - sphinx cryptographic packet format tests
// Copyright (C) 2018  David Stainton.

extern crate rand;
extern crate sphinxcrypto;
//extern crate rustc_serialize;

//use self::rustc_serialize::hex::ToHex;
use std::any::Any;
use self::rand::Rng;
use self::rand::os::OsRng;


use sphinxcrypto::server::sphinx_packet_unwrap;
use sphinxcrypto::ecdh::PrivateKey;
use sphinxcrypto::client::{new_packet, PathHop, new_surb, new_packet_from_surb, decrypt_surb_payload};
use sphinxcrypto::constants::{NUMBER_HOPS, NODE_ID_SIZE, FORWARD_PAYLOAD_SIZE, RECIPIENT_ID_SIZE, SURB_ID_SIZE, PAYLOAD_SIZE};
use sphinxcrypto::commands::{Delay, Recipient, SURBReply};


struct NodeParams {
    pub id: [u8; NODE_ID_SIZE],
    pub private_key: PrivateKey,
}

fn os_rng() -> OsRng {
    OsRng::new().expect("failure to create an OS RNG")
}

fn new_node<R: Rng>(rng: &mut R) -> NodeParams {
    let mut id = [0u8; NODE_ID_SIZE];
    rng.fill_bytes(&mut id);
    let keypair = PrivateKey::generate(rng).unwrap();
    return NodeParams{
        id: id,
        private_key: keypair,
    };
}

fn new_path_vector<R: Rng>(rng: &mut R, num_hops: u8, is_surb: bool) -> (Vec<NodeParams>, Vec<PathHop>) {
    const DELAY_BASE: u32 = 123;

    // Generate the keypairs and node identifiers for the "nodes".
    let mut nodes = vec![];
    let mut i = 0;
    while i < num_hops {
        nodes.push(new_node(rng));
        i += 1;
    }

    // Assemble the path vector.
    let mut path = vec![];
    i = 0;
    while i < num_hops {
        let mut commands: Vec<Box<Any>> = vec![];
        if i < num_hops - 1 {
            // Non-terminal hop, add the delay.
            let delay = Delay {
                delay: DELAY_BASE * (i as u32 + 1),
            };
            commands.push(Box::new(delay));
        } else {
	    // Terminal hop, add the recipient.
            let mut rcpt_id = [0u8; RECIPIENT_ID_SIZE];
            rng.fill_bytes(&mut rcpt_id);
            let rcpt = Recipient {
                id: rcpt_id,
            };
            commands.push(Box::new(rcpt));

            if is_surb {
                let mut surb_id = [0u8; SURB_ID_SIZE];
                rng.fill_bytes(&mut surb_id);
                let surb_reply = SURBReply {
                    id: surb_id,
                };
                commands.push(Box::new(surb_reply));
            }
        }
        let hop = PathHop {
            id: nodes[i as usize].id,
            public_key: nodes[i as usize].private_key.public_key(),
            commands: Some(commands),
        };
        path.push(hop);
        i += 1;
    }
    return (nodes, path);
}

#[test]
fn sphinx_forward_test() {
    let mut payload = [0u8; FORWARD_PAYLOAD_SIZE];
    let s = String::from("We must defend our own privacy if we expect to have any. \
                          We must come together and create systems which allow anonymous transactions to take place. \
                          People have been defending their own privacy for centuries with whispers, darkness, envelopes, \
                          closed doors, secret handshakes, and couriers. The technologies of the past did not allow for strong \
                          privacy, but electronic technologies do.");
    let _s_len = s.len();
    let string_bytes = s.into_bytes();
    payload[.._s_len].copy_from_slice(&string_bytes);

    // Generate the "nodes" and path for the forward sphinx packet.
    let mut r = os_rng();
    let is_surb = false;

    let mut num_hops = 1;
    while num_hops < NUMBER_HOPS {
        let _tuple = new_path_vector(&mut r, num_hops as u8, is_surb);
        let nodes = _tuple.0;
        let path = _tuple.1;

	// Create the packet.
        let _packet_result = new_packet(&mut r, path, payload);
        let mut packet = _packet_result.unwrap();

        // Unwrap the packet, validating the output.
        let mut i = 0;
        while i < num_hops {
            let _unwrap_tuple = sphinx_packet_unwrap(&nodes[i].private_key, &mut packet);
            let option_cmds = _unwrap_tuple.2;
            let err = _unwrap_tuple.3;
            let final_payload = _unwrap_tuple.0;
            let cmds = option_cmds.unwrap();

            assert!(err.is_none());
            if i == nodes.len() - 1 {
                assert!(cmds.len() == 1);
                assert_eq!(final_payload.unwrap().as_slice(), &payload[..]);
                assert!(cmds[0].downcast_ref::<Recipient>().is_some());
            } else {
                assert!(cmds.len() == 2);
                assert!(final_payload.is_none());
                assert!(cmds[0].downcast_ref::<Delay>().is_some());
            }
            i += 1;
        }
        num_hops += 1;
    }

} // end of fn sphinx_packet_unwrap_test() {

#[test]
fn sphinx_surb_test() {
    let mut payload = [0u8; FORWARD_PAYLOAD_SIZE];
    let s = String::from("We must defend our own privacy if we expect to have any. \
                          We must come together and create systems which allow anonymous transactions to take place. \
                          People have been defending their own privacy for centuries with whispers, darkness, envelopes, \
                          closed doors, secret handshakes, and couriers. The technologies of the past did not allow for strong \
                          privacy, but electronic technologies do.");
    let _s_len = s.len();
    let string_bytes = s.into_bytes();
    payload[.._s_len].copy_from_slice(&string_bytes);

    let mut r = os_rng();
    let is_surb = true;
    let mut num_hops = 1;
    while num_hops < NUMBER_HOPS {
        let _tuple = new_path_vector(&mut r, num_hops as u8, is_surb);
        let nodes = _tuple.0;
        let path = _tuple.1;
        let (surb, surb_keys) = new_surb(&mut r, path).unwrap();
        let (mut packet, _next_mix) = new_packet_from_surb(surb, payload).unwrap();
        let mut i = 0;
        while i < num_hops {
            let _unwrap_tuple = sphinx_packet_unwrap(&nodes[i].private_key, &mut packet);
            let option_cmds = _unwrap_tuple.2;
            let err = _unwrap_tuple.3;
            let final_payload_res = _unwrap_tuple.0;
            let cmds = option_cmds.unwrap();
            assert!(err.is_none());
            assert!(cmds.len() == 2);
            if i == nodes.len() - 1 {
                let mut _payload = [0u8; PAYLOAD_SIZE];
                _payload.copy_from_slice(final_payload_res.unwrap().as_slice());
                let _result = decrypt_surb_payload(_payload, surb_keys.clone());
                assert!(_result.is_ok());
                //assert_eq!(plaintext, payload.to_vec());
            }
            i += 1;
        }
        num_hops += 1;
    }
}