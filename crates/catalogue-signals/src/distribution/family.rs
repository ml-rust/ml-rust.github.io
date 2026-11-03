//! The family of a Repology repo name.

/// Trailing repo name segments that name a channel or an architecture.
///
/// A digits-only segment is a version and is stripped without being listed.
const CHANNEL_SEGMENTS: [&str; 24] = [
    "unstable",
    "testing",
    "edge",
    "rawhide",
    "stable",
    "current",
    "legacy",
    "next",
    "oldstable",
    "rolling",
    "cauldron",
    "cooker",
    "develop",
    "dev",
    "master",
    "leap",
    "tumbleweed",
    "amber",
    "byzantium",
    "landing",
    "x86",
    "clang64",
    "clangarm64",
    "ucrt64",
];

/// The family of the Repology repo name `repo`.
///
/// The name splits on `_`. The rule strips trailing segments while a segment is
/// all digits or one of the listed channel and architecture words. The first
/// segment always stays. `debian_12` and `debian_unstable` give `debian`.
/// `ubuntu_24_04` gives `ubuntu`. `openwrt_24_10_x86_64` gives `openwrt`.
/// `homebrew` and `crates_io` stay as they are.
#[must_use]
pub fn family(repo: &str) -> &str {
    let mut name = repo;
    while let Some((head, tail)) = name.rsplit_once('_') {
        if !is_version_or_channel(tail) {
            break;
        }
        name = head;
    }
    name
}

/// Whether `segment` is a version number or a channel word.
fn is_version_or_channel(segment: &str) -> bool {
    (!segment.is_empty() && segment.bytes().all(|b| b.is_ascii_digit()))
        || CHANNEL_SEGMENTS.contains(&segment)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions_and_channels_are_stripped() {
        for (repo, expected) in [
            ("debian_12", "debian"),
            ("debian_unstable", "debian"),
            ("fedora_39", "fedora"),
            ("fedora_rawhide", "fedora"),
            ("ubuntu_24_04", "ubuntu"),
            ("alpine_3_21", "alpine"),
            ("alpine_edge", "alpine"),
            ("nix_stable_24_05", "nix"),
            ("nix_unstable", "nix"),
            ("opensuse_leap_15_6", "opensuse"),
            ("opensuse_tumbleweed", "opensuse"),
            ("openwrt_24_10_x86_64", "openwrt"),
            ("void_x86_64", "void"),
            ("msys2_ucrt64", "msys2"),
            ("pkgsrc_2026_3", "pkgsrc"),
            ("pkgsrc_current", "pkgsrc"),
            ("raspbian_oldstable", "raspbian"),
            ("kali_rolling", "kali"),
        ] {
            assert_eq!(family(repo), expected, "{repo}");
        }
    }

    #[test]
    fn plain_names_and_non_channel_suffixes_stay() {
        for repo in [
            "homebrew",
            "arch",
            "crates_io",
            "kiss_community",
            "alt_p11",
            "msys2",
        ] {
            assert_eq!(family(repo), repo);
        }
    }

    #[test]
    fn the_first_segment_always_stays() {
        assert_eq!(family("stable"), "stable");
        assert_eq!(family("2026_1"), "2026");
        assert_eq!(family(""), "");
        assert_eq!(family("_12"), "");
    }
}
