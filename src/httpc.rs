//! A small HTTP client over Pingora's connector and OpenSSL, for conduit's own calls: the ACME
//! CA, CrowdSec's local API and its AppSec component. One request at a time per call, whole
//! bodies, optional keep-alive with one retry when a pooled connection turns out stale.

use std::net::SocketAddr;
use std::time::Duration;

use bytes::Bytes;
use http::{HeaderMap, HeaderName, HeaderValue, Method, StatusCode, Uri, header};
use pingora::connectors::ConnectorOptions;
use pingora::connectors::http::Connector as HttpConnector;
use pingora::http::RequestHeader;
use pingora::upstreams::peer::HttpPeer;
use pingora::{Error, ErrorType, Result};

pub struct Reply {
    pub status: StatusCode,
    pub headers: HeaderMap,
    pub body: Bytes,
}

pub struct Client {
    connector: HttpConnector,
    timeout: Duration,
    max_body: usize,
}

/// How long an idle connection stays in the pool.
const IDLE: Duration = Duration::from_secs(30);

impl Client {
    /// `ca_file` replaces the system CA store for HTTPS; `timeout` bounds connect, each read
    /// and each write; `pool` idle connections are kept for keep-alive requests.
    pub fn new(ca_file: Option<&str>, timeout: Duration, max_body: usize, pool: usize) -> Client {
        let mut o = ConnectorOptions::new(pool);
        o.ca_file = ca_file.map(str::to_string);
        Client {
            connector: HttpConnector::new(Some(o)),
            timeout,
            max_body,
        }
    }

    async fn resolve(uri: &Uri) -> Result<(Vec<SocketAddr>, bool, String)> {
        let tls = match uri.scheme_str() {
            Some("https") => true,
            Some("http") => false,
            _ => return Error::e_explain(ErrorType::InternalError, format!("URL {uri}")),
        };
        let Some(host) = uri.host() else {
            return Error::e_explain(ErrorType::InternalError, format!("URL {uri}"));
        };
        let host = host.trim_start_matches('[').trim_end_matches(']');
        let port = uri.port_u16().unwrap_or(if tls { 443 } else { 80 });
        let addrs = match host.parse::<std::net::IpAddr>() {
            Ok(ip) => vec![SocketAddr::new(ip, port)],
            Err(_) => tokio::net::lookup_host((host, port))
                .await
                .map_err(|e| Error::because(ErrorType::ConnectError, format!("resolve {host}"), e))?
                .collect(),
        };
        Ok((addrs, tls, host.to_string()))
    }

    /// Send a request and read the whole response. `headers` come after `Host`; the client
    /// sets `Content-Length` and `Connection` itself.
    pub async fn send(
        &self,
        method: Method,
        uri: &Uri,
        headers: &[(HeaderName, HeaderValue)],
        body: Bytes,
        keepalive: bool,
    ) -> Result<Reply> {
        let (addrs, tls, host) = Self::resolve(uri).await?;
        let mut last = None;
        for addr in addrs {
            let mut peer = HttpPeer::new(addr, tls, host.clone());
            let o = &mut peer.options;
            o.connection_timeout = Some(self.timeout);
            o.total_connection_timeout = Some(self.timeout);
            o.read_timeout = Some(self.timeout);
            o.write_timeout = Some(self.timeout);
            o.idle_timeout = Some(IDLE);
            // A pooled connection may have been closed by the server meanwhile: one more try
            // on a fresh one.
            for _ in 0..2 {
                match self
                    .exchange(&peer, &method, uri, headers, &body, keepalive)
                    .await
                {
                    Ok(r) => return Ok(r),
                    Err((e, true)) => last = Some(e),
                    Err((e, false)) => {
                        last = Some(e);
                        break;
                    }
                }
            }
        }
        Err(last.unwrap_or_else(|| {
            Error::explain(ErrorType::ConnectError, format!("{host} has no address"))
        }))
    }

    /// One attempt; the error says whether another connection may succeed where this one
    /// failed (a reused connection the server had closed).
    async fn exchange(
        &self,
        peer: &HttpPeer,
        method: &Method,
        uri: &Uri,
        headers: &[(HeaderName, HeaderValue)],
        body: &Bytes,
        keepalive: bool,
    ) -> std::result::Result<Reply, (Box<Error>, bool)> {
        let (mut session, reused) = self
            .connector
            .get_http_session(peer)
            .await
            .map_err(|e| (e, false))?;
        // The peer's read and write timeouts only bound connecting: a session has its own.
        session.set_read_timeout(Some(self.timeout));
        session.set_write_timeout(Some(self.timeout));
        // A reused connection the server closed meanwhile fails at once; a timeout means
        // the server is slow, and another connection would only wait as long again.
        let fail = |e: Box<Error>| {
            let stale = reused
                && !matches!(
                    e.etype(),
                    ErrorType::ReadTimedout | ErrorType::WriteTimedout
                );
            (e, stale)
        };
        let path = uri.path_and_query().map_or("/", |p| p.as_str());
        let mut req =
            RequestHeader::build(method.clone(), path.as_bytes(), Some(headers.len() + 3))
                .map_err(fail)?;
        let authority = uri.authority().map_or("localhost", |a| a.as_str());
        req.insert_header(header::HOST, authority).map_err(fail)?;
        for (name, value) in headers {
            req.append_header(name.clone(), value.clone())
                .map_err(fail)?;
        }
        if !body.is_empty() || *method == Method::POST {
            req.insert_header(header::CONTENT_LENGTH, body.len())
                .map_err(fail)?;
        }
        if !keepalive {
            req.insert_header(header::CONNECTION, "close")
                .map_err(fail)?;
        }
        session
            .write_request_header(Box::new(req))
            .await
            .map_err(fail)?;
        if !body.is_empty() {
            session
                .write_request_body(body.clone(), true)
                .await
                .map_err(fail)?;
        }
        session.finish_request_body().await.map_err(fail)?;
        session.read_response_header().await.map_err(fail)?;
        // From here on the request reached the server: never resend it.
        let fatal = |e: Box<Error>| (e, false);
        let Some(head) = session.response_header() else {
            return Err(fatal(Error::explain(
                ErrorType::InvalidHTTPHeader,
                "no response header",
            )));
        };
        let (status, resp_headers) = (head.status, head.headers.clone());
        let declared = resp_headers
            .get(header::CONTENT_LENGTH)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse::<usize>().ok())
            .unwrap_or(0);
        let mut data = Vec::with_capacity(declared.min(self.max_body));
        if *method != Method::HEAD {
            while let Some(chunk) = session.read_response_body().await.map_err(fatal)? {
                if data.len() + chunk.len() > self.max_body {
                    return Err(fatal(Error::explain(
                        ErrorType::InvalidHTTPHeader,
                        "response too large",
                    )));
                }
                data.extend_from_slice(&chunk);
            }
        }
        if keepalive {
            self.connector
                .release_http_session(session, peer, Some(IDLE))
                .await;
        }
        Ok(Reply {
            status,
            headers: resp_headers,
            body: Bytes::from(data),
        })
    }
}
