# [Wireskip](https://wireskip.com/)

A work in progress generic tunneling (VPN-like) solution using HTTP 2/3 as transport loosely based on the [IETF MASQUE WG](https://ietf-wg-masque.github.io/) effort but not affiliated with the IETF in any way. Built on top of [hyper](https://github.com/hyperium/hyper) as a single self-contained binary.

## Why?

HTTP 2/3 is an inconspicuous transport layer which enables [collateral freedom](https://en.wikipedia.org/wiki/Collateral_freedom) when it is used to circumvent network censorship. Our goal is for Wireskip traffic to be indistinguishable from regular web browsing to an outside observer.

## Roadmap

Currently implemented features:

- [RFC 9113: HTTP/2 CONNECT method](https://datatracker.ietf.org/doc/html/rfc9113#name-the-connect-method)
- [RFC 9298: Proxying UDP in HTTP](https://datatracker.ietf.org/doc/html/rfc9298)
- [RFC 9297: HTTP Datagrams and the Capsule Protocol](https://datatracker.ietf.org/doc/html/rfc9297)
- [RFC 1928: SOCKS Protocol Version 5](https://datatracker.ietf.org/doc/html/rfc1928): `CONNECT`, `UDP ASSOCIATE` commands
- Arbitrary number of onion-routed hops before arriving at the target; no relay knows your entire circuit

Planned for the future / needs a helping hand:

- [RFC 9484: Proxying IP in HTTP](https://datatracker.ietf.org/doc/html/rfc9484)
- The SOCKSv5 code is very barebones and needs to be improved
- An easy built-in way to deploy to cloud instances via `ssh`
- System-wide traffic tunneling through `tun` device
- Authentication mechanisms to control access
- Unit / integration test coverage
- User-friendly platform apps
- Docs!

## Contributing

- Please note that code is far from stable yet
- Nightly Rust und unstable features are OK
- Be very careful about adding new deps
- Do one thing and do it well
- Use `clippy` and `rustfmt`

In `contrib/test.sh` you will find a very simple testing scenario for tunneling TCP and UDP through a local 3-relay circuit.

## Discussion

If you have any questions, feel free to [join our Discord](https://discord.gg/fyvKG4gAUe)!
