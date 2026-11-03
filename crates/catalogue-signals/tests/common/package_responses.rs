//! Canned `.crate` downloads.

use flate2::Compression;
use flate2::write::GzEncoder;
use tar::{EntryType, Header};

use crate::transport::FakeResponse;

impl FakeResponse {
    /// A response with `status` and a binary `body`.
    pub fn bytes(status: u16, body: Vec<u8>) -> Self {
        Self {
            status,
            headers: Vec::new(),
            body,
        }
    }

    /// A `200` whose body is the `.crate` of `name` and `version`.
    ///
    /// `vcs_info` is the content of `.cargo_vcs_info.json`. `None` omits the file.
    pub fn package(name: &str, version: &str, vcs_info: Option<&str>) -> Self {
        let mut files = vec![("Cargo.toml", "[package]\n")];
        if let Some(json) = vcs_info {
            files.insert(0, (".cargo_vcs_info.json", json));
        }
        let mut builder = tar::Builder::new(GzEncoder::new(Vec::new(), Compression::fast()));
        for (file, contents) in files {
            let mut header = Header::new_gnu();
            header.set_entry_type(EntryType::Regular);
            header.set_size(contents.len() as u64);
            header.set_mode(0o644);
            builder
                .append_data(
                    &mut header,
                    format!("{name}-{version}/{file}"),
                    contents.as_bytes(),
                )
                .expect("file entry");
        }
        let body = builder
            .into_inner()
            .expect("tar stream")
            .finish()
            .expect("gzip stream");
        Self::bytes(200, body)
    }
}
