pub const TCP: &str = "tcp";
pub const UDP: &str = "udp";

pub const PREFIX: &str = "wireskip";

pub const TARGET: &str = "wireskip-target";
pub const REASON: &str = "wireskip-reason";

pub type Target = url::Url;

#[cfg(test)]
mod tests {
    #[test]
    fn test_consts() {
        assert_eq!(super::TARGET, super::PREFIX.to_owned() + "-target");
        assert_eq!(super::REASON, super::PREFIX.to_owned() + "-reason");
    }
}
