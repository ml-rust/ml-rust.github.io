//! Response bodies recorded from the release, Homebrew, Docker Hub and Repology
//! APIs, trimmed to the fields the lookups read plus a few neighbours.

/// Four releases of `BurntSushi/ripgrep` behind a draft. Each release keeps a
/// binary archive, a Debian package and the checksum files that go with them.
pub const RIPGREP_RELEASES: &str = r#"[
 {"tag_name":"15.3.0-rc","published_at":null,"draft":true,"prerelease":false,
  "assets":[{"name":"ripgrep-15.3.0-x86_64-apple-darwin.tar.gz","download_count":777}]},
 {"tag_name":"15.2.0","published_at":"2026-07-15T16:26:10Z","draft":false,"prerelease":false,
  "assets":[
   {"name":"ripgrep-15.2.0-aarch64-apple-darwin.tar.gz","download_count":1519119},
   {"name":"ripgrep-15.2.0-aarch64-apple-darwin.tar.gz.sha256","download_count":45895},
   {"name":"ripgrep-15.2.0-x86_64-unknown-linux-musl.tar.gz","download_count":4041926},
   {"name":"ripgrep-15.2.0-x86_64-unknown-linux-musl.tar.gz.sha256","download_count":119713},
   {"name":"ripgrep_15.2.0-1_amd64.deb","download_count":18964},
   {"name":"ripgrep_15.2.0-1_amd64.deb.sha256","download_count":2682}]},
 {"tag_name":"15.1.0","published_at":"2025-10-22T13:00:26Z","draft":false,"prerelease":false,
  "assets":[
   {"name":"ripgrep-15.1.0-aarch64-apple-darwin.tar.gz","download_count":1891104},
   {"name":"ripgrep-15.1.0-aarch64-apple-darwin.tar.gz.sha256","download_count":18385},
   {"name":"ripgrep-15.1.0-x86_64-unknown-linux-musl.tar.gz","download_count":12139029},
   {"name":"ripgrep-15.1.0-x86_64-unknown-linux-musl.tar.gz.sha256","download_count":915255},
   {"name":"ripgrep_15.1.0-1_amd64.deb","download_count":118418},
   {"name":"ripgrep_15.1.0-1_amd64.deb.sha256","download_count":3470}]},
 {"tag_name":"15.0.0-beta","published_at":"2025-10-16T03:17:57Z","draft":false,"prerelease":true,
  "assets":[
   {"name":"ripgrep-15.0.0-aarch64-apple-darwin.tar.gz","download_count":41095},
   {"name":"ripgrep-15.0.0-x86_64-unknown-linux-musl.tar.gz","download_count":429645},
   {"name":"ripgrep_15.0.0-1_amd64.deb","download_count":2428},
   {"name":"SHA256SUMS","download_count":3000},
   {"name":"checksums.txt","download_count":3000}]},
 {"tag_name":"14.1.1","published_at":"2024-09-01T00:00:00Z","draft":false,"prerelease":false,
  "assets":[{"name":"ripgrep-14.1.1-x86_64-apple-darwin.tar.gz","download_count":99}]}
]"#;

/// The `ripgrep` formula analytics, with the `--HEAD` variant keys.
pub const RIPGREP_FORMULA: &str = r#"{"name":"ripgrep","tap":"homebrew/core",
 "analytics":{
  "install":{
   "30d":{"ripgrep":47071,"ripgrep --HEAD":43},
   "90d":{"ripgrep":149042,"ripgrep --HEAD":158},
   "365d":{"ripgrep":702403,"ripgrep --HEAD":778}},
  "install_on_request":{
   "30d":{"ripgrep":36219,"ripgrep --HEAD":43},
   "90d":{"ripgrep":114336,"ripgrep --HEAD":158},
   "365d":{"ripgrep":504258,"ripgrep --HEAD":778}},
  "build_error":{"30d":{"ripgrep":6}}}}"#;

/// A formula that has no analytics block.
pub const NO_ANALYTICS_FORMULA: &str = r#"{"name":"fresh","tap":"homebrew/core"}"#;

/// The `library/nginx` repo.
pub const NGINX_REPO: &str = r#"{"user":"library","name":"nginx","namespace":"library",
 "pull_count":13546273780,"last_updated":"2026-11-03T04:52:35.923567Z","star_count":21416}"#;

/// A repo that omits `last_updated`.
pub const NEW_REPO: &str = r#"{"user":"acme","name":"tool","pull_count":12,"star_count":0}"#;

/// A repo whose `last_updated` is null.
pub const NULL_UPDATED_REPO: &str = r#"{"name":"tool","pull_count":12,"last_updated":null}"#;

/// Package objects of `ripgrep`, trimmed to one row per distinct repo.
pub const RIPGREP_PACKAGES: &str = r#"[
 {"repo":"debian_12","srcname":"rust-ripgrep","version":"13.0.0","status":"outdated"},
 {"repo":"debian_12","srcname":"ripgrep","version":"13.0.0","status":"outdated"},
 {"repo":"debian_unstable","srcname":"rust-ripgrep","version":"14.1.1","status":"outdated"},
 {"repo":"fedora_39","srcname":"ripgrep","version":"14.0.3","status":"outdated"},
 {"repo":"homebrew","srcname":"ripgrep","version":"15.2.0","status":"newest"},
 {"repo":"ubuntu_24_04","srcname":"rust-ripgrep","version":"14.1.0","status":"outdated"},
 {"repo":"alpine_edge","srcname":"ripgrep","version":"15.2.0","status":"newest"},
 {"repo":"alpine_3_21","srcname":"ripgrep","version":"14.1.1","status":"outdated"}
]"#;
