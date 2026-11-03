//! Builders for in-memory `db-dump.tar.gz` archives.

use flate2::Compression;
use flate2::write::GzEncoder;
use tar::{EntryType, Header};

/// The top-level directory of every built archive.
pub const DIRECTORY: &str = "2026-03-02-020017";

/// An ordered list of archive files, relative to [`DIRECTORY`].
pub struct DumpBuilder {
    files: Vec<(String, String)>,
}

impl DumpBuilder {
    /// Replaces the file at `path`, or appends it when absent.
    pub fn with(mut self, path: &str, contents: &str) -> Self {
        match self.files.iter_mut().find(|(p, _)| p == path) {
            Some(file) => file.1 = contents.to_owned(),
            None => self.files.push((path.to_owned(), contents.to_owned())),
        }
        self
    }

    /// Appends a file at `path`, even when one already exists there.
    pub fn appended(mut self, path: &str, contents: &str) -> Self {
        self.files.push((path.to_owned(), contents.to_owned()));
        self
    }

    /// Replaces or appends `data/<table>.csv`.
    pub fn table(self, table: &str, csv: &str) -> Self {
        self.with(&format!("data/{table}.csv"), csv)
    }

    /// Removes the file at `path`.
    pub fn without(mut self, path: &str) -> Self {
        self.files.retain(|(p, _)| p != path);
        self
    }

    /// Moves the file at `path` to the end of the archive.
    pub fn moved_last(mut self, path: &str) -> Self {
        let index = self
            .files
            .iter()
            .position(|(p, _)| p == path)
            .expect("file present");
        let file = self.files.remove(index);
        self.files.push(file);
        self
    }

    /// The gzip-compressed tar bytes, with a directory entry first.
    pub fn build(&self) -> Vec<u8> {
        let mut builder = tar::Builder::new(GzEncoder::new(Vec::new(), Compression::fast()));
        let mut dir = Header::new_gnu();
        dir.set_entry_type(EntryType::Directory);
        dir.set_size(0);
        dir.set_mode(0o755);
        builder
            .append_data(&mut dir, format!("{DIRECTORY}/"), std::io::empty())
            .expect("directory entry");
        for (path, contents) in &self.files {
            let mut header = Header::new_gnu();
            header.set_entry_type(EntryType::Regular);
            header.set_size(contents.len() as u64);
            header.set_mode(0o644);
            builder
                .append_data(
                    &mut header,
                    format!("{DIRECTORY}/{path}"),
                    contents.as_bytes(),
                )
                .expect("file entry");
        }
        builder
            .into_inner()
            .expect("tar stream")
            .finish()
            .expect("gzip stream")
    }
}

/// A valid dump in the crates.io table order, with ignored files and tables mixed in.
pub fn valid() -> DumpBuilder {
    DumpBuilder { files: Vec::new() }
        .with(
            "metadata.json",
            r#"{"timestamp":"2026-03-02T02:00:17.123456Z","crates_io_commit":"0123abc"}"#,
        )
        .with("README.md", "# crates.io dump\n")
        .with("export.sql", "COPY crates TO 'data/crates.csv';\n")
        .table("categories", CATEGORIES)
        .table("crate_downloads", "crate_id,downloads\n1,100\n")
        .table("crates", CRATES)
        .table("keywords", KEYWORDS)
        .table("metadata", "total_downloads\n12345\n")
        .table("reserved_crate_names", "name\nstd\n")
        .table("teams", TEAMS)
        .table("users", USERS)
        .table("crates_categories", "category_id,crate_id\n2,1\n1,1\n9,2\n")
        .table("crates_keywords", "crate_id,keyword_id\n1,5\n2,77\n")
        .table("crate_owners", CRATE_OWNERS)
        .table("deleted_crates", DELETED_CRATES)
        .table("versions", VERSIONS)
        .table(
            "default_versions",
            "crate_id,num_versions,version_id\n1,2,11\n2,1,20\n3,1,30\n",
        )
        .table("dependencies", DEPENDENCIES)
        .table("version_downloads", VERSION_DOWNLOADS)
}

const CATEGORIES: &str = "category,crates_cnt,created_at,description,id,path,slug\n\
Science,2,2017-01-17 19:13:05.112025+00,\"Science, math, and \"\"stuff\"\"\",1,root.science,science\n\
Machine learning,1,2017-01-17 19:13:05.112025+00,ML,2,root.science.ml,science::ml\n";

const KEYWORDS: &str = "crates_cnt,created_at,id,keyword\n1,2017-01-17 19:13:05+00,5,tensor\n";

const CRATES: &str = "created_at,description,documentation,homepage,id,max_features,max_upload_size,name,readme,repository,updated_at,zz_added_later\n\
2025-01-01 10:00:00.5+00,\"Tensors, FFT and linalg\",\"\",,1,,,numr,\"# numr\n\nFast tensors, with \"\"quotes\"\", commas, and\nmultiple lines.\",\"\",2026-02-01 00:00:00+00,x\n\
2020-05-06 07:08:09.1+00,,,https://old.example,2,,,oldcrate,\"# oldcrate\n\n> This crate is no longer maintained.\n\",https://github.com/example/oldcrate,2021-01-01 00:00:00+00,x\n\
2026-02-20 00:00:00+00,\"\",,,3,,,helper,,,2026-02-20 00:00:00+00,x\n";

const TEAMS: &str = "avatar,github_id,id,login,name,org_id\n,1,3,github:ml-rust:core,Core,9\n";

const USERS: &str = "gh_avatar,gh_id,id,name,publish_notifications,username\n,11,7,Alice,t,alice\n";

const CRATE_OWNERS: &str = "crate_id,created_at,created_by,owner_id,owner_kind\n\
1,2025-01-01 10:00:00+00,7,7,0\n\
1,2025-01-01 10:00:00+00,,3,1\n\
2,2020-05-06 07:08:09+00,,7,0\n";

const DELETED_CRATES: &str = "available_at,created_at,deleted_at,deleted_by,id,message,min_version,name\n\
2026-03-01 12:00:00+00,2020-01-01 00:00:00+00,2026-02-01 12:00:00+00,,1,\"spam, sorry\",,gone\n";

const VERSIONS: &str = "bin_names,checksum,crate_id,crate_size,created_at,downloads,edition,features,has_lib,id,license,links,num,published_by,rust_version,updated_at,yanked\n\
{},aa,1,10,2025-01-01 10:00:00+00,5,2021,\"{\"\"default\"\":[\"\"std\"\"],\"\"std\"\":[]}\",t,10,MIT,,0.1.0,7,1.80,2025-01-01 10:00:00+00,f\n\
{},bb,1,10,2026-02-01 00:00:00+00,5,2024,{},t,11,MIT,,0.2.0,7,,2026-02-01 00:00:00+00,f\n\
{},cc,2,10,2020-05-06 07:08:09+00,5,,{},t,20,MIT,,1.0.0,7,\"\",2020-05-06 07:08:09+00,t\n\
{},dd,3,10,2026-02-20 00:00:00+00,5,2024,{},t,30,MIT,,0.1.0,,1.85,2026-02-20 00:00:00+00,f\n";

const DEPENDENCIES: &str = "crate_id,default_features,explicit_name,features,id,kind,optional,req,target,version_id\n\
1,t,,{},1,0,f,^0.1,,10\n\
1,t,,\"{serde,std}\",2,0,f,^0.2,,30\n\
2,f,old,{},3,2,t,=1.0.0,cfg(unix),30\n\
2,t,,{},4,1,f,1,,11\n";

const VERSION_DOWNLOADS: &str = "date,downloads,version_id\n\
2026-03-02,5,11\n\
2026-03-02,7,10\n\
2026-03-01,2,30\n\
2026-02-28,1,11\n\
2026-02-27,1000,11\n\
2026-03-01,4,999\n\
2026-01-01,4,998\n";
