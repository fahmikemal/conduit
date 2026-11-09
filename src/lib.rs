//! conduit: a reverse proxy built on Pingora. The binary (`src/main.rs`) wires these
//! modules into a Pingora server; the library exists so fuzz targets (`fuzz/`) can reach
//! the parsers and request handling directly. It is not a published API.

#![allow(clippy::len_without_is_empty)]

pub mod accesslog;
pub mod acme;
pub mod admin;
pub mod auth;
pub mod cache;
pub mod config;
pub mod crowdsec;
pub mod exploits;
pub mod files;
#[doc(hidden)]
pub mod fuzzing;
pub mod httpc;
pub mod limits;
pub mod logger;
pub mod metrics;
pub mod pathnorm;
pub mod plugin;
pub mod proxy;
pub mod realip;
pub mod reload;
pub mod replace;
pub mod route;
pub mod rules;
pub mod runtime;
pub mod timefmt;
pub mod tls;
pub mod units;
pub mod upstream;
