use crossbeam::channel::unbounded;
use std::collections::HashMap;
use std::thread;
use std::time::Duration;
use wg_internal::drone::Drone;
use wg_internal::network::{NodeId, SourceRoutingHeader};
use wg_internal::packet::{FloodRequest, FloodResponse, NodeType};
use wg_internal::packet::{Packet, PacketType};

use crate::assert_matches_any;

/* THE FOLLOWING TESTS CHECKS IF YOUR DRONE IS HANDLING CORRECTLY PACKETS (FLOOD REQUESTS/RESPONSES) */

const TIMEOUT: Duration = Duration::from_millis(400);

fn create_sample_flood_req(flood_id: u64, initiator_id : NodeId ,path_trace: Vec<(NodeId, NodeType)>) -> Packet {
    Packet {
        pack_type: PacketType::FloodRequest(FloodRequest {
            flood_id,
            initiator_id,
            path_trace,
        }),
        routing_header: SourceRoutingHeader {
            hop_index: 0,
            hops: Vec::new(),
        },
        session_id: 1,
    }
}

fn create_flood_res(
    flood_id: u64,
    path_trace: Vec<(NodeId, NodeType)>,
    routing_header: SourceRoutingHeader,
) -> Packet {
    let flood_res = FloodResponse {
        flood_id,
        path_trace,
    };
    Packet::new_flood_response(routing_header, 1, flood_res)
}

/// This function checks whether a drone builds a flood response packet correctly when drone has no neighbours (except for the receiver).
pub fn generic_new_flood<T: Drone + Send + 'static>() {
    // Client 1
    let (c_send, c_recv) = unbounded::<Packet>();
    // Drone 11
    let (d_send, d_recv) = unbounded();
    // SC commands
    let (_d_command_send, d_command_recv) = unbounded();
    let (d_event_send, _d_event_recv) = unbounded();

    let mut drone = T::new(
        11,
        d_event_send.clone(),
        d_command_recv,
        d_recv.clone(),
        HashMap::from([(1, c_send.clone())]),
        0.0,
    );

    thread::spawn(move || {
        drone.run();
    });

    let msg = create_sample_flood_req(1, 1, vec![(1, NodeType::Client)]);
    // Client sends packet to d
    d_send.send(msg.clone()).unwrap();

    let flood_res = create_flood_res(
        1,
        vec![(1, NodeType::Client), (11, NodeType::Drone)],
        SourceRoutingHeader::new(vec![11, 1], 1),
    );
    // Client receive a flood response originated from 'd'
    assert_eq!(c_recv.recv_timeout(TIMEOUT).unwrap(), flood_res);
}

/// This functions checks if a drone handles correctly a flood request when:
/// - the drone has no neighbours (except for the receiver)
/// - the `initiator_id` has not been included into the path trace.
pub fn generic_new_flood_no_initiator<T: Drone + Send + 'static>() {
    // Client 1
    let (c_send, c_recv) = unbounded::<Packet>();
    // Drone 11
    let (d_send, d_recv) = unbounded();
    // SC commands
    let (_d_command_send, d_command_recv) = unbounded();
    let (d_event_send, _d_event_recv) = unbounded();

    let mut drone = T::new(
        11,
        d_event_send.clone(),
        d_command_recv,
        d_recv.clone(),
        HashMap::from([(1, c_send.clone())]),
        0.0,
    );

    thread::spawn(move || {
        drone.run();
    });

    let msg = create_sample_flood_req(1, 1, vec![]);
    // Client sends packet to d
    d_send.send(msg.clone()).unwrap();

    let flood_res = create_flood_res(
        1,
        vec![(11, NodeType::Drone)],
        SourceRoutingHeader::new(vec![11, 1], 1),
    );

    // Client receive a flood response originated from 'd'
    assert_eq!(c_recv.recv_timeout(TIMEOUT).unwrap(), flood_res);
}

/// This function checks if a flood request is forwarded to all neighbours of a drone (excluding the sender) and waits for 2 responses.
/// ### Network Topology
/// C(1) -> D(11)
/// D(11) -> C(1), D(12), D(13)
/// D(12) -> D(11)
/// D(13) -> D(11)
pub fn generic_new_flood_neighbours<T: Drone + Send + 'static>() {
    let (c_send, c_recv) = unbounded::<Packet>();
    let (d_send, d_recv) = unbounded();
    let (d2_send, d2_recv) = unbounded::<Packet>();
    let (d3_send, d3_recv) = unbounded::<Packet>();
    // SC commands
    let (_d_command_send, d_command_recv) = unbounded();
    let (d_event_send, _d_event_recv) = unbounded();
        
    let neighbours = HashMap::from([
        (1, c_send.clone()),
        (12, d2_send.clone()),
        (13, d3_send.clone()),
    ]);
    let mut drone = T::new(
        11,
        d_event_send.clone(),
        d_command_recv.clone(),
        d_recv.clone(),
        neighbours,
        0.0,
    );

    thread::spawn(move || {
        drone.run();
    });

    let neighbours = HashMap::from([(11, d_send.clone())]);
    let mut drone2 = T::new(
        12,
        d_event_send.clone(),
        d_command_recv.clone(),
        d2_recv.clone(),
        neighbours,
        0.0,
    );

    thread::spawn(move || {
        drone2.run();
    });

    let neighbours = HashMap::from([(11, d_send.clone())]);
    let mut drone3 = T::new(
        13,
        d_event_send.clone(),
        d_command_recv.clone(),
        d3_recv.clone(),
        neighbours,
        0.0,
    );

    thread::spawn(move || {
        drone3.run();
    });

    let msg = create_sample_flood_req(1, 1, vec![(1, NodeType::Client)]);
    // Client sends packet to d
    d_send.send(msg.clone()).unwrap();

    let flood_res_d12 = create_flood_res(
        1,
        vec![
            (1, NodeType::Client),
            (11, NodeType::Drone),
            (12, NodeType::Drone),
        ],
        SourceRoutingHeader::new(vec![12, 11, 1], 2),
    );
    let flood_res_d13 = create_flood_res(
        1,
        vec![
            (1, NodeType::Client),
            (11, NodeType::Drone),
            (13, NodeType::Drone),
        ],
        SourceRoutingHeader::new(vec![13, 11, 1], 2),
    );

    // Client receive 2 flood responses originated from `d12` and `d13`
    for _ in 0..2 {
        let res = c_recv.recv_timeout(TIMEOUT);
        if res.is_err() {
            panic!(
                "assertion `left == right` failed:\nleft: `{:?}`\nright1: `{:?}`\nright2: `{:?}`",
                res, flood_res_d12, flood_res_d13
            );
        }
        let res = res.unwrap();
        assert_matches_any!(res, flood_res_d12, flood_res_d13);
    }
}

/// This function checks if a drone forwards correctly a flood response packet to the next hop.
pub fn generic_flood_res_forward<T: Drone + Send + 'static>() {
    let (d2_send, d2_recv) = unbounded();
    let (d3_send, d3_recv) = unbounded();
    // SC commands
    let (_d_command_send, d_command_recv) = unbounded();
    let (d_event_send, _d_event_recv) = unbounded();

    let mut drone_2 = T::new(
        2,
        d_event_send.clone(),
        d_command_recv,
        d2_recv,
        HashMap::from([(3, d3_send.clone())]),
        0.0,
    );

    thread::spawn(move || {
        drone_2.run();
    });

    let mut flood_res = create_flood_res(
        1,
        vec![(1, NodeType::Client), (2, NodeType::Drone)],
        SourceRoutingHeader::new(vec![1, 2, 3], 1),
    );
    d2_send.send(flood_res.clone()).unwrap();

    flood_res.routing_header.hop_index += 1;

    assert_eq!(d3_recv.recv_timeout(TIMEOUT).unwrap(), flood_res);
}

/// This function checks if a drone handles correctly a flood request when the `flood_id` and the `initiator_id` are known.
/// ### Network Topology
/// C(1) -> D(11) 
/// C(2) -> D(11)
/// D(11) -> C(1), C(2), D(12)
pub fn generic_known_flood_req<T: Drone + Send + 'static>() {
    // Client 1
    let (c_send, c_recv) = unbounded::<Packet>();
    // Drone 11
    let (d_send, d_recv) = unbounded();
    // Drone 12
    let (d12_send, d12_recv) = unbounded();
    // SC commands
    let (_d_command_send, d_command_recv) = unbounded();
    let (d_event_send, _d_event_recv) = unbounded();

    let mut drone = T::new(
        11,
        d_event_send.clone(),
        d_command_recv.clone(),
        d_recv.clone(),
        HashMap::from([(1, c_send.clone()), (12, d12_send.clone())]),
        0.0,
    );

    let mut drone2 = T::new(
        12,
        d_event_send.clone(),
        d_command_recv.clone(),
        d12_recv.clone(),
        HashMap::from([(11, d_send.clone())]),
        0.0,
    );

    thread::spawn(move || {
        drone.run();
    });
    thread::spawn(move || {
        drone2.run();
    });

    let msg = create_sample_flood_req(1, 1, vec![(1, NodeType::Client)]);
    // Client sends packet to d
    d_send.send(msg.clone()).unwrap();
    thread::sleep(Duration::from_millis(300));
    d_send.send(msg.clone()).unwrap();

    let flood_res_d11 = create_flood_res(
        1,
        vec![(1, NodeType::Client), (11, NodeType::Drone)],
        SourceRoutingHeader::new(vec![11, 1], 1),
    );

    let flood_res_d12 = create_flood_res(
        1,
        vec![(1, NodeType::Client), (11, NodeType::Drone), (12, NodeType::Drone)],
        SourceRoutingHeader::new(vec![12, 11, 1], 2),
    );
    
    for _ in 0..2 {
        let res = c_recv.recv_timeout(TIMEOUT);
        if res.is_err() {
            panic!(
                "assertion `left == right` failed:\nleft: `{:?}`\nright1: `{:?}`\nright2: `{:?}`",
                res, flood_res_d11, flood_res_d12
            );
        }
        let res = res.unwrap();
        assert_matches_any!(res, flood_res_d11, flood_res_d12);
    }
}

/// This function checks if a drone handles correctly two flood requests with the same `flood_id` but different `initiator_id`.
/// ### Network Topology
/// C(1) -> D(11) -> D(12)
pub fn generic_flood_req_two_initiator<T: Drone + Send + 'static>() {
    let (d11_send, d11_recv) = unbounded();
    let (d12_send, d12_recv) = unbounded();
    // SC commands
    let (_d_command_send, d_command_recv) = unbounded();
    let (d_event_send, _d_event_recv) = unbounded();

    let mut drone = T::new(
        11,
        d_event_send.clone(),
        d_command_recv,
        d11_recv,
        HashMap::from([(12, d12_send.clone())]),
        0.0,
    );

    thread::spawn(move || {
        drone.run();
    });

    // Client(1) sends a flood request to drone(11) with flood_id = 1 and initiator_id = 1
    let mut msg_c1 = create_sample_flood_req(1, 1, vec![(1, NodeType::Client)]);
    d11_send.send(msg_c1.clone()).unwrap();

    // Client(2) sends a flood request to drone(11) with flood_id = 1 and initiator_id = 2
    let mut msg_c2 = create_sample_flood_req(1, 2, vec![(2, NodeType::Client)]);
    d11_send.send(msg_c2.clone()).unwrap();

    // Drone(12) receives two flood requests with Drone(11) added to the path trace
    let expected_d12 = create_sample_flood_req(1, 1, vec![(1, NodeType::Client), (11, NodeType::Drone)]);
    let expected_d12_2 = create_sample_flood_req(1, 2, vec![(2, NodeType::Client), (11, NodeType::Drone)]);

    for _ in 0..2 {
        let res = d12_recv.recv_timeout(TIMEOUT);
        if res.is_err() {
            panic!(
                "assertion `left == right` failed:\nleft: `{:?}`\nright1: `{:?}`\nright2: `{:?}`",
                res, expected_d12, expected_d12_2
            );
        }
        let res = res.unwrap();
        assert_matches_any!(res, expected_d12, expected_d12_2);
    }
}