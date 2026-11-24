//! The project's name on the wire. dsp-kitchen and lucem each put their name into the protocol:
//! the TLS application protocols (`dspk-stream/3`, `dspk-pair/1`), the SPAKE2 identities of
//! pairing (`dspk-pair-client`), the browser's WebTransport path (`/dspk`) and the name of the
//! server's certificate authority (`dspk serve authority`). Changing any of them would break
//! clients already paired, so each project sets its own once, at start-up, and kitchen-net uses
//! them everywhere:
//!
//! ```ignore
//! kitchen_net::set_service(kitchen_net::Service { name: "dspk", alpn: b"dspk-stream/3", pair_alpn: b"dspk-pair/1" });
//! ```

use std::sync::OnceLock;

/// What names a project on the wire.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Service {
    /// Short name (`dspk`, `lucem`): pairing identities, paths, the authority's name.
    pub name: &'static str,
    /// The session protocol (ALPN) and its version, so QUIC refuses peers of another version.
    pub alpn: &'static [u8],
    /// The pairing protocol (ALPN): a computer without a certificate connects with it to pair.
    pub pair_alpn: &'static [u8],
}

impl Service {
    /// The service used when a project set none.
    pub const DEFAULT: Service = Service { name: "kitchen", alpn: b"kitchen-stream/1", pair_alpn: b"kitchen-pair/1" };

    /// SPAKE2 identity of the computer's side of pairing.
    pub fn pair_client_id(&self) -> String {
        format!("{}-pair-client", self.name)
    }

    /// SPAKE2 identity of the server's side of pairing.
    pub fn pair_server_id(&self) -> String {
        format!("{}-pair-server", self.name)
    }

    /// The name a computer connects with to pair (the server is not yet trusted).
    pub fn pairing_server_name(&self) -> String {
        format!("{}-pairing", self.name)
    }

    /// The common name of the server's certificate authority. Issuing rebuilds the CA's
    /// parameters from it, so it must stay the same for an authority's whole life.
    pub fn authority_name(&self) -> String {
        format!("{} serve authority", self.name)
    }

    /// The common name of the server's own certificate.
    pub fn server_name(&self) -> String {
        format!("{} serve", self.name)
    }

    /// The WebTransport session's path on the server, for browsers.
    pub fn webtransport_path(&self) -> String {
        format!("/{}", self.name)
    }

    /// What a page's loader asks first (relative to the page): a server answers with
    /// [`crate::pair::ServeInfo`]; a page on its own ships `{"server": null}`.
    pub fn serve_info_path(&self) -> String {
        format!("/{}-serve.json", self.name)
    }
}

static SERVICE: OnceLock<Service> = OnceLock::new();

/// Sets the project's service. The first call wins; returns whether `service` is now the service
/// (the same one again is harmless, a different one is refused).
pub fn set_service(service: Service) -> bool {
    *SERVICE.get_or_init(|| service) == service
}

/// The project's service, or [`Service::DEFAULT`].
pub fn service() -> Service {
    SERVICE.get().copied().unwrap_or(Service::DEFAULT)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_follow_the_project() {
        let dspk = Service { name: "dspk", alpn: b"dspk-stream/3", pair_alpn: b"dspk-pair/1" };
        assert_eq!(dspk.pair_client_id(), "dspk-pair-client");
        assert_eq!(dspk.authority_name(), "dspk serve authority");
        assert_eq!(dspk.webtransport_path(), "/dspk");
        assert_eq!(dspk.serve_info_path(), "/dspk-serve.json");
    }
}
