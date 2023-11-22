use hyper::ext::Protocol;

// max supported UDP packet size as per RFC 9298
pub const UDP_MAX: usize = 65527;

pub const CONNECT_UDP: Protocol = Protocol::from_static("connect-udp");
