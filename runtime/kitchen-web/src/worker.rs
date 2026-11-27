//! The engine worker's side: an app's own [`Runtime`] answering the page's calls, encoded as on
//! the wire. The app's WebAssembly module (loaded by `web/engine-worker.js`) exports an `Engine`
//! whose `answer(bytes)` is [`answer`] over its runtime; this is plain Rust, the same in a browser
//! and in a test.

use kitchen_client::{Runtime, RuntimeError};
use prost::Message as _;

/// The answer to one encoded request (a `kitchen.runtime.v1.Request`), encoded: what the worker
/// posts back. A request that cannot be read is answered with why.
pub fn answer(runtime: &dyn Runtime, bytes: &[u8]) -> Vec<u8> {
    let result = match kitchen_wire::parse_request(bytes) {
        Ok((caller, request)) => runtime.call(&caller, request),
        Err(e) => Err(RuntimeError::Invalid(format!("not a request: {e}"))),
    };
    kitchen_wire::encode_answer(&result).encode_to_vec()
}

#[cfg(test)]
mod tests {
    use super::*;
    use kitchen_client::{Answer, Caller, Request, Reply};

    struct Types;

    impl Runtime for Types {
        fn call(&self, caller: &Caller, request: Request) -> Reply<Answer> {
            match request {
                Request::Types => Ok(Answer::Welcome { contract: "0.1".into(), capabilities: vec![], apps: vec![caller.by.clone()] }),
                _ => Err(RuntimeError::Unsupported("only types".into())),
            }
        }
    }

    #[test]
    fn a_worker_answers_what_a_server_would() {
        let me = Caller::new("you", "page");
        let ask = kitchen_wire::encode_request(&me, &Request::Types).encode_to_vec();
        assert!(matches!(kitchen_wire::parse_answer(&answer(&Types, &ask)).unwrap(), Ok(Answer::Welcome { apps, .. }) if apps == ["you"]));
        let other = kitchen_wire::encode_request(&me, &Request::RunCancel { run: "r".into() }).encode_to_vec();
        assert!(matches!(kitchen_wire::parse_answer(&answer(&Types, &other)).unwrap(), Err(RuntimeError::Unsupported(_))));
        assert!(matches!(kitchen_wire::parse_answer(&answer(&Types, b"\xff\xff")).unwrap(), Err(RuntimeError::Invalid(_))));
    }
}
